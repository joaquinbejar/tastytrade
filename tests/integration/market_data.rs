//! REST market-data snapshots.
//!
//! The encoding is the point. Every other array parameter in this crate is a
//! repeated key; this endpoint takes **one key per instrument type**, each a
//! comma-separated list. Getting it backwards returns one symbol per type and
//! looks like thin data rather than a client bug.

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

async fn venue() -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /market-data/by-type".to_string(),
        Route::ok(include_str!("../../Doc/market_data_by_type.json")),
    );
    MockVenue::start(routes).await
}

fn last_query(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/market-data"))
        .expect("a market-data request must have been sent")
        .target
        .split_once('?')
        .map(|(_, query)| query.to_string())
        .unwrap_or_default()
}

/// One key per type, each sent once, comma-joined — and explicitly **not**
/// repeated keys.
#[tokio::test]
async fn each_instrument_type_is_one_parameter_sent_once() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .market_data_by_type(
            &MarketDataRequest::new()
                .with_equities(&["AAPL", "TSLA"])
                .with_cryptocurrencies(&["BTC/USD"])
                .with_indices(&["SPX"]),
        )
        .await;

    let query = last_query(&venue);
    assert_eq!(
        query,
        "index=SPX&equity=AAPL%2CTSLA&cryptocurrency=BTC%2FUSD"
    );
    assert_eq!(query.matches("equity=").count(), 1, "sent once: {query}");
    assert!(!query.contains("equity%5B%5D"), "not an array key: {query}");
}

/// The venue's own payload, through the real transport, with prices as
/// `Decimal` and not `f64`.
#[tokio::test]
async fn the_venues_own_snapshot_decodes() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let snapshots = client
        .market_data_by_type(&MarketDataRequest::new().with_equities(&["AAPL"]))
        .await
        .expect("the snapshot must decode");

    let bitcoin = snapshots
        .iter()
        .find(|snapshot| snapshot.symbol == "BTC/USD")
        .expect("the fixture carries a cryptocurrency");
    assert_eq!(bitcoin.bid.expect("a bid").to_string(), "94005.47");
    assert_eq!(bitcoin.beta, None, "a cryptocurrency has no beta");
}

/// A local refusal sends nothing at all — asserted by the absence of the
/// request, not only by the error.
#[tokio::test]
async fn an_over_large_request_never_reaches_the_venue() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let too_many: Vec<String> = (0..101).map(|i| format!("SYM{i}")).collect();
    let error = client
        .market_data_by_type(&MarketDataRequest::new().with_equities(&too_many))
        .await
        .expect_err("101 symbols is over the limit");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| !request.target.starts_with("/market-data")),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// The limit counts every type together, which is the part a caller building
/// one watchlist per type would get wrong.
#[tokio::test]
async fn the_limit_spans_instrument_types() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let half: Vec<String> = (0..50).map(|i| format!("SYM{i}")).collect();
    let at_limit = MarketDataRequest::new()
        .with_equities(&half)
        .with_futures(&half);
    assert!(client.market_data_by_type(&at_limit).await.is_ok());

    let over = at_limit.with_indices(&["SPX"]);
    assert!(client.market_data_by_type(&over).await.is_err());
}
