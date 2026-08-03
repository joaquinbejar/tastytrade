//! The account websocket, driven end to end against a loopback venue.
//!
//! These exist because the account streamer had **no reconnect coverage at
//! all** while the quote streamer had five tests for its replay — which is how
//! a reconnect that could never work survived three PRs through that file.
//!
//! Nothing here touches the network: `MockVenue` answers the OAuth exchange and
//! the account listing, `WsVenue` is the streamer socket, and both are loopback
//! sockets this process owns.

use std::collections::HashMap;
use std::time::Duration;

use tastytrade::prelude::*;
use tastytrade::streaming::reconnect::{BackoffPolicy, ConnectionState};
use tastytrade::utils::config::TastyTradeConfig;

use crate::support::{MockVenue, Route, WsVenue, one_account_body, sentinel, token_response_body};

/// Reconnects fast enough for a test to watch, and gives up rather than
/// spinning if something is wrong.
fn quick_policy() -> BackoffPolicy {
    BackoffPolicy {
        initial: Duration::from_millis(20),
        max_delay: Duration::from_millis(50),
        max_attempts: Some(5),
        jitter: 0.0,
    }
}

/// A REST venue that can authenticate and list one account.
async fn rest_venue() -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(one_account_body(sentinel::ACCOUNT_NUMBER)),
    );
    MockVenue::start(routes).await
}

fn config_for(rest: &MockVenue, ws: &WsVenue) -> TastyTradeConfig {
    TastyTradeConfig {
        client_secret: sentinel::CLIENT_SECRET.into(),
        refresh_token: sentinel::REFRESH_TOKEN.into(),
        client_id: String::new(),
        redirect_uri: String::new(),
        use_demo: true,
        log_level: "WARN".to_string(),
        base_url: rest.base_url().to_string(),
        websocket_url: ws.url().to_string(),
    }
}

/// The regression this file exists for.
///
/// The supervisor used to replay by queueing an action and awaiting an
/// acknowledgement only a running session could send — at a point in the loop
/// where no session was running. It parked forever, so the stream never came
/// back, for every caller who had subscribed to an account.
#[tokio::test]
async fn a_reconnect_restores_the_subscription_it_was_watching() {
    let rest = rest_venue().await;
    // Close the first connection after one frame: the `connect` this test
    // sends. That is the drop the client has to recover from.
    let ws = WsVenue::start(1).await;
    let config = config_for(&rest, &ws);

    let tasty = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");
    let accounts = tasty.accounts().await.expect("one account");
    let streamer = AccountStreamer::connect_with_policy(&tasty, quick_policy())
        .await
        .expect("the loopback venue accepts the connection");

    streamer
        .subscribe_to_account(&accounts[0])
        .await
        .expect("the venue acknowledges the subscription");

    // The venue dropped that connection. A second one has to arrive carrying
    // the subscription again, without anybody asking for it.
    ws.wait_for("the reconnect to replay the subscription", |venue| {
        venue.connections() >= 2 && !venue.actions_on(2).is_empty()
    })
    .await;

    let replayed: Vec<_> = ws
        .frames()
        .into_iter()
        .filter(|frame| frame.connection == 2)
        .collect();

    assert!(
        replayed.iter().any(|frame| frame.action == "connect"),
        "the new session must restore what was being watched: {replayed:?}"
    );
    assert!(
        replayed
            .iter()
            .any(|frame| frame.value.contains(sentinel::ACCOUNT_NUMBER)),
        "and it must be the same account: {replayed:?}"
    );
}

/// `Connected` has to mean what it says. It used to be set one line before the
/// session started, while the comment beside the replay claimed it was set
/// only once what was being watched was watched again.
#[tokio::test]
async fn connected_is_claimed_only_once_the_subscription_is_restored() {
    let rest = rest_venue().await;
    let ws = WsVenue::start(1).await;
    let config = config_for(&rest, &ws);

    let tasty = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");
    let accounts = tasty.accounts().await.expect("one account");
    let streamer = AccountStreamer::connect_with_policy(&tasty, quick_policy())
        .await
        .expect("the loopback venue accepts the connection");

    streamer
        .subscribe_to_account(&accounts[0])
        .await
        .expect("the venue acknowledges the subscription");

    ws.wait_for("the reconnect to replay the subscription", |venue| {
        venue.connections() >= 2 && !venue.actions_on(2).is_empty()
    })
    .await;

    // The venue acknowledges, so the restoration completes and the state
    // follows it. A bounded wait rather than an assertion on a single read:
    // the supervisor is a separate task and the point is that it *gets* there.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if matches!(streamer.state().await, ConnectionState::Connected) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!(
        "the streamer never reported Connected after restoring; last state {:?}",
        streamer.state().await
    );
}

/// A first connection has nothing to restore, so it must not invent a
/// subscription the caller never asked for — and must still report itself
/// connected.
#[tokio::test]
async fn a_first_connection_subscribes_to_nothing_on_its_own() {
    let rest = rest_venue().await;
    let ws = WsVenue::start(usize::MAX).await;
    let config = config_for(&rest, &ws);

    let tasty = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");
    let streamer = AccountStreamer::connect_with_policy(&tasty, quick_policy())
        .await
        .expect("the loopback venue accepts the connection");

    // Long enough for a heartbeat to be impossible and a stray connect to
    // have shown up if one were going to.
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        !ws.frames().iter().any(|frame| frame.action == "connect"),
        "nothing was subscribed, so nothing may be restored: {:?}",
        ws.frames()
    );
    assert!(matches!(streamer.state().await, ConnectionState::Connected));
}
