//! Market metrics, dividends and earnings.
//!
//! One property above all: `symbols` is **comma-joined into one parameter**,
//! not repeated keys. Getting it wrong returns metrics for one symbol, which
//! reads as a thin answer rather than a client bug.

use std::collections::HashMap;

use chrono::NaiveDate;
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

fn items(body: &str) -> String {
    format!(r#"{{"data": {{"items": [{body}]}}, "context": "/market-metrics"}}"#)
}

fn last_target(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/market-metrics"))
        .expect("a market-metrics request must have been sent")
        .target
}

/// The trap. `symbols` is one comma-joined parameter, unlike the repeated
/// `symbol[]` keys the instrument listings use.
#[tokio::test]
async fn symbols_reach_the_venue_comma_joined_and_sent_once() {
    let venue = venue_with(vec![("GET /market-metrics", Route::ok(items("")))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client.market_metrics(&["AAPL", "TSLA", "SPY"]).await;

    let target = last_target(&venue);
    assert_eq!(target, "/market-metrics?symbols=AAPL%2CTSLA%2CSPY");
    assert_eq!(target.matches("symbols=").count(), 1, "sent once: {target}");
    assert!(!target.contains("symbols%5B%5D"), "not an array: {target}");
}

/// A metric decodes with its per-expiration block, and the expiration stays a
/// calendar day even though the schema types it as a timestamp.
#[tokio::test]
async fn a_metric_decodes_with_its_expirations() {
    let venue = venue_with(vec![(
        "GET /market-metrics",
        Route::ok(items(
            r#"{"symbol": "AAPL", "implied-volatility-index": "0.3421",
                "implied-volatility-rank": "0.5117", "liquidity-rating": 4,
                "option-expiration-implied-volatilities": [
                    {"expiration-date": "2026-05-15T00:00:00.000-04:00",
                     "settlement-type": "PM", "option-chain-type": "Standard",
                     "implied-volatility": "0.29"}]}"#,
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let metrics = client
        .market_metrics(&["AAPL"])
        .await
        .expect("the metric must decode");

    assert_eq!(metrics.len(), 1);
    assert_eq!(
        metrics[0]
            .implied_volatility_index
            .expect("an index")
            .to_string(),
        "0.3421"
    );
    let expiration = &metrics[0].option_expiration_implied_volatilities[0];
    assert_eq!(
        expiration.expiration_date,
        NaiveDate::from_ymd_opt(2026, 5, 15),
        "an expiration is a calendar day whichever shape the venue sends"
    );
}

/// The symbol is a path segment, so a class separator has to be encoded.
#[tokio::test]
async fn a_dividend_lookup_encodes_its_symbol() {
    let venue = venue_with(vec![(
        "GET /market-metrics/historic-corporate-events/dividends/BRK%2FB",
        Route::ok(items(r#"{"occurred-date": "2026-02-10", "amount": 0.25}"#)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let dividends = client
        .historic_dividends("BRK/B")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(dividends.len(), 1);
    assert_eq!(dividends[0].amount.expect("an amount").to_string(), "0.25");
}

/// `start-date` is required by the venue, and the type makes it impossible to
/// leave out.
#[tokio::test]
async fn an_earnings_query_always_carries_its_start_date() {
    let venue = venue_with(vec![(
        "GET /market-metrics/historic-corporate-events/earnings-reports/AAPL",
        Route::ok(items(r#"{"occurred-date": "2026-02-01", "eps": "-1.25"}"#)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let reports = client
        .historic_earnings(
            "AAPL",
            &EarningsRange::from(NaiveDate::from_ymd_opt(2026, 1, 1).expect("a real date")),
        )
        .await
        .expect("the reports must decode");

    assert!(last_target(&venue).ends_with("?start-date=2026-01-01"));
    // A loss is a real figure.
    assert_eq!(reports[0].eps.expect("an eps").to_string(), "-1.25");

    let _ = client
        .historic_earnings(
            "AAPL",
            &EarningsRange::between(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("a real date"),
                NaiveDate::from_ymd_opt(2026, 3, 31).expect("a real date"),
            ),
        )
        .await;
    assert!(
        last_target(&venue).ends_with("?start-date=2026-01-01&end-date=2026-03-31"),
        "{}",
        last_target(&venue)
    );
}
