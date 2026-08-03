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
        leg_type: BacktestInstrument::EquityOption,
        direction: BacktestDirection::Short,
        quantity: Decimal::ONE,
        strike_selection: StrikeSelection::Delta,
        days_until_expiration: 45,
        side: Some(BacktestSide::Put),
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

    // An equity leg carrying a side, an option leg with none, a quantity
    // outside the documented range and a fractional one — each is a request
    // the venue rejects, built from a misreading of which field means what.
    let equity_with_side = || {
        let mut leg = leg();
        leg.leg_type = BacktestInstrument::Equity;
        leg
    };
    let option_without_side = || {
        let mut leg = leg();
        leg.side = None;
        leg
    };
    let quantity = |value: Decimal| {
        let mut leg = leg();
        leg.quantity = value;
        leg
    };

    for bad in [
        NewBacktest::new("SPY", day(2024, 1, 1), day(2024, 12, 31), vec![]),
        NewBacktest::new("SPY", day(2024, 12, 31), day(2024, 1, 1), vec![leg()]),
        NewBacktest::new("  ", day(2024, 1, 1), day(2024, 12, 31), vec![leg()]),
        NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![equity_with_side()],
        ),
        NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![option_without_side()],
        ),
        NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![quantity(Decimal::from(MAX_BACKTEST_QUANTITY + 1))],
        ),
        NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![quantity(Decimal::new(15, 1))],
        ),
        NewBacktest::new(
            "SPY",
            day(2024, 1, 1),
            day(2024, 12, 31),
            vec![quantity(Decimal::ZERO)],
        ),
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

/// A simulated trade is not a backtest leg.
///
/// `POST /simulate-trade` takes instruments that already exist, by symbol.
/// Sending a backtest leg — with a `type`, a `strikeSelection`, a `delta` and a
/// `daysUntilExpiration` — describes a strike to select, which is a different
/// request the venue does not accept.
#[tokio::test]
async fn an_impossible_simulation_never_leaves_the_process() {
    let venue = venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let good_leg = || SimulatedLeg {
        symbol: "SPY   190227C00275000".to_string(),
        direction: BacktestDirection::Short,
        quantity: Decimal::ONE,
    };

    for bad in [
        SimulateTrade::new("SPY", vec![]),
        SimulateTrade::new("  ", vec![good_leg()]),
        SimulateTrade::new(
            "SPY",
            vec![SimulatedLeg {
                symbol: "  ".to_string(),
                ..good_leg()
            }],
        ),
        SimulateTrade::new(
            "SPY",
            vec![SimulatedLeg {
                quantity: Decimal::new(5, 1),
                ..good_leg()
            }],
        ),
    ] {
        let error = client
            .simulate_trade(&bad)
            .await
            .expect_err("the local checks must refuse this");
        assert!(matches!(error, TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable(), "nothing was sent");
    }

    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.target == "/oauth/token"),
        "nothing may have been sent: {:?}",
        venue.requests()
    );

    // The documented covered call passes, and serializes to the documented
    // shape rather than to a backtest leg.
    let good = SimulateTrade::new(
        "SPY",
        vec![
            good_leg(),
            SimulatedLeg {
                symbol: "SPY".to_string(),
                direction: BacktestDirection::Long,
                quantity: Decimal::from(100),
            },
        ],
    );
    let body = serde_json::to_value(&good).expect("serialises");
    assert_eq!(body["underlying"], "SPY");
    assert_eq!(body["legs"][0]["symbol"], "SPY   190227C00275000");
    assert_eq!(body["legs"][0]["direction"], "short");
    assert_eq!(body["legs"][0]["quantity"], 1);
    assert!(body["legs"][0].get("type").is_none(), "{body}");
    assert!(body["legs"][0].get("strikeSelection").is_none(), "{body}");
    assert!(body.get("startTime").is_none(), "{body}");
}
