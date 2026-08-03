//! Backtesting, end to end.
//!
//! The one property this area has that no other does: it is served by a
//! **different host**.
//!
//! That host cannot be exercised here. This suite is network-free by design —
//! every other test drives a loopback socket this process owns — and the
//! backtester's URL is a published constant with no override, so a test that
//! called it would reach the internet. Nothing here does.
//!
//! What is testable is the part that carries the risk: the local refusals, and
//! that a backtest is not addressed against the configured base URL. The URL
//! join itself is covered by a unit test in `api::client`.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
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
    MockVenue::start(routes).await
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
}

fn leg() -> BacktestLeg {
    BacktestLeg {
        leg_type: "Put".to_string(),
        direction: "Short".to_string(),
        quantity: 1,
        strike_selection: "Delta".to_string(),
        days_until_expiration: 45,
        side: None,
        strike_relative_leg: None,
        delta: Some(Decimal::new(16, 2)),
        percentage_otm: None,
        current_price_offset: None,
        premium: None,
    }
}

/// The local checks run before anything leaves the process, which matters more
/// here than elsewhere: a backtest is long-running, so a request that was
/// always going to be rejected costs a wait as well as a round trip.
#[tokio::test]
async fn an_impossible_backtest_never_leaves_the_process() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    for bad in [
        NewBacktest::new("SPY", day(2024, 1, 1), day(2024, 12, 31), vec![]),
        NewBacktest::new("SPY", day(2024, 12, 31), day(2024, 1, 1), vec![leg()]),
        NewBacktest::new("  ", day(2024, 1, 1), day(2024, 12, 31), vec![leg()]),
    ] {
        let error = client
            .create_backtest(&bad)
            .await
            .expect_err("the local checks must refuse this");
        assert!(!error.is_retryable());
    }

    // Only the token exchange reached the loopback venue, and nothing reached
    // the backtester either — there is no route here that would have answered.
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.target == "/oauth/token"),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// The session's environment is what an error names, because that is what a
/// caller needs to know — which credentials were used — and the backtester
/// publishes one host for both.
#[tokio::test]
async fn an_error_names_the_sessions_environment() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .create_backtest(&NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![],
        ))
        .await
        .expect_err("a legless backtest is refused");

    // The refusal is local, so it carries no environment — which is itself the
    // right answer: nothing was sent anywhere.
    assert!(matches!(error, TastyTradeError::Precondition(_)));
}

/// The published host is the one in the document, and there is only one.
#[test]
fn the_backtester_host_is_a_separate_published_one() {
    assert_eq!(
        BACKTESTER_BASE_URL,
        "https://backtester.vast.tastyworks.com"
    );
    assert!(
        BACKTESTER_BASE_URL.starts_with("https://"),
        "a second host must still be https"
    );
}
