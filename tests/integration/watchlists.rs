//! Watchlists, end to end.
//!
//! This is the only area besides orders where a client can **destroy** user
//! data, so the tests are about the shape of the mutating calls: what `PUT`
//! actually sends, that a blank name never leaves the process, and that the
//! name reaches the path encoded.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;

use crate::support::{MockVenue, Route, sentinel, token_response_body};

fn config_for(venue: &MockVenue) -> TastyTradeConfig {
    TastyTradeConfig {
        client_secret: sentinel::CLIENT_SECRET.into(),
        refresh_token: sentinel::REFRESH_TOKEN.into(),
        client_id: "client-abc".to_string(),
        redirect_uri: "https://app.example.com/cb".to_string(),
        use_demo: true,
        log_level: "WARN".to_string(),
        base_url: venue.base_url().to_string(),
        websocket_url: "ws://127.0.0.1:1".to_string(),
    }
}

async fn venue_with(extra: Vec<(&str, Route)>) -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    for (key, route) in extra {
        routes.insert(key.to_string(), route);
    }
    MockVenue::start(routes).await
}

const LIST: &str = r#"{"name": "My List", "group-name": "mine", "order-index": 1,
                       "watchlist-entries": [{"symbol": "AAPL", "instrument-type": "Equity"}]}"#;

fn single(body: &str) -> String {
    format!(r#"{{"data": {body}, "context": "/watchlists"}}"#)
}

/// A curated list name contains spaces, which is exactly the case the shared
/// path encoder exists for.
#[tokio::test]
async fn a_watchlist_name_is_encoded_into_the_path() {
    let venue = venue_with(vec![(
        "GET /public-watchlists/High%20Options%20Volume",
        Route::ok(single(LIST)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let list = client
        .public_watchlist("High Options Volume")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(list.watchlist_entries.len(), 1);
}

/// `counts-only` is sent only when asked for, so the venue's own default
/// survives an ordinary call.
#[tokio::test]
async fn counts_only_is_sent_only_when_asked_for() {
    let venue = venue_with(vec![(
        "GET /public-watchlists",
        Route::ok(format!(
            r#"{{"data": {{"items": [{LIST}]}}, "context": "/public-watchlists"}}"#
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client.public_watchlists(false).await;
    assert_eq!(
        venue.requests().last().expect("a request").target,
        "/public-watchlists"
    );

    let _ = client.public_watchlist_counts().await;
    assert_eq!(
        venue.requests().last().expect("a request").target,
        "/public-watchlists?counts-only=true"
    );
}

/// The `PUT` body is the whole list. That is what makes it a replacement and
/// not an append, and it is the thing a caller most needs to see.
#[tokio::test]
async fn replacing_sends_the_entire_list() {
    let venue = venue_with(vec![("PUT /watchlists/My%20List", Route::ok(single(LIST)))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .replace_watchlist(
            "My List",
            &NewWatchlist::new("My List", &["TSLA"]).with_group_name("mine"),
        )
        .await;

    let sent = venue
        .requests()
        .into_iter()
        .rfind(|request| request.method == "PUT")
        .expect("a PUT must have been sent");
    assert_eq!(sent.target, "/watchlists/My%20List");

    let body: serde_json::Value = serde_json::from_str(&sent.body).expect("a JSON body");
    assert_eq!(body["name"], "My List");
    assert_eq!(body["watchlist-entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["watchlist-entries"][0]["symbol"], "TSLA");
    assert_eq!(body["group-name"], "mine");
    // `cms-id` is a read-side field and is not part of the create body — sending
    // it as null would be a different request from not sending it.
    assert!(body.get("cms-id").is_none(), "{body}");
    assert!(body.get("order-index").is_none(), "{body}");
}

/// A blank name never leaves the process. The name is also the URL segment a
/// later replace or delete addresses, so a list nobody can name is a list
/// nobody can remove.
#[tokio::test]
async fn a_blank_name_never_reaches_the_venue() {
    let venue = venue_with(vec![("POST /watchlists", Route::ok(single(LIST)))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .create_watchlist(&NewWatchlist::new("   ", &["AAPL"]))
        .await
        .expect_err("a blank name is refused");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.target != "/watchlists"),
        "nothing may have been sent: {:?}",
        venue.requests()
    );

    // A blank symbol on an entry is refused for the same reason.
    assert!(
        client
            .create_watchlist(&NewWatchlist::new("Fine", &["  "]))
            .await
            .is_err()
    );
}

/// Deletion is its own method with its own argument, so it cannot be reached
/// from a listing by accident — and it goes out as a `DELETE`.
#[tokio::test]
async fn deleting_uses_its_own_verb_and_path() {
    let venue = venue_with(vec![(
        "DELETE /watchlists/My%20List",
        Route::ok(single(LIST)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let deleted = client
        .delete_watchlist("My List")
        .await
        .expect("the list must be deleted");

    assert_eq!(deleted.name, "My List");
    let sent = venue
        .requests()
        .into_iter()
        .rfind(|request| request.method == "DELETE")
        .expect("a DELETE must have been sent");
    assert_eq!(sent.target, "/watchlists/My%20List");
}

/// Pairs watchlists keep their equations as they arrived, since the venue
/// publishes no schema for them.
#[tokio::test]
async fn a_pairs_watchlist_keeps_its_equations() {
    let venue = venue_with(vec![(
        "GET /pairs-watchlists",
        Route::ok(
            r#"{"data": {"items": [{"name": "Pairs", "order-index": 2,
                                    "pairs-equations": [{"left-symbol": "AAPL",
                                                         "right-symbol": "MSFT"}]}]},
                "context": "/pairs-watchlists"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let lists = client
        .pairs_watchlists()
        .await
        .expect("the lists must decode");

    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].pairs_equations.len(), 1);
    assert_eq!(lists[0].pairs_equations[0]["left-symbol"], "AAPL");
}
