//! Market sessions and holidays.
//!
//! Two properties: the repeated `instrument-collections[]` selection reaches
//! the venue, and the nine-month range limit is enforced before anything is
//! sent.

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

fn last_target(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/market-time"))
        .expect("a market-time request must have been sent")
        .target
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
}

/// The required selection is repeated keys, and the constructor makes an empty
/// one unrepresentable.
#[tokio::test]
async fn the_current_session_sends_repeated_collections() {
    let venue = venue_with(vec![(
        "GET /market-time/sessions/current",
        Route::ok(
            r#"{"data": {"state": "Open", "instrument-collection": "Equity",
                         "open-at": "2026-08-03T09:30:00.000-04:00",
                         "close-at": "2026-08-03T16:00:00.000-04:00"},
                "context": "/market-time/sessions/current"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let session = client
        .current_market_session(InstrumentCollection::Equity, &[InstrumentCollection::Cme])
        .await
        .expect("the session must decode");

    assert_eq!(
        last_target(&venue),
        "/market-time/sessions/current\
         ?instrument-collections%5B%5D=Equity&instrument-collections%5B%5D=CME"
    );
    // The offset survives, which is the point of preserving it.
    assert_eq!(
        session.open_at.expect("an open").offset().local_minus_utc(),
        -4 * 3600
    );
    assert_eq!(session.state.as_deref(), Some("Open"));
}

/// The nine-month limit is local, so an over-long range costs no round trip.
#[tokio::test]
async fn an_over_long_range_never_reaches_the_venue() {
    let venue = venue_with(vec![(
        "GET /market-time/sessions",
        Route::ok(r#"{"data": {"items": []}, "context": "/market-time/sessions"}"#),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .market_sessions(&SessionRange::between(day(2026, 1, 1), day(2027, 1, 1)))
        .await
        .expect_err("a year is more than nine months");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| !request.target.starts_with("/market-time")),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// `to-date` is required and always sent; the rest are omitted when unset.
#[tokio::test]
async fn a_range_sends_what_it_was_given() {
    let venue = venue_with(vec![(
        "GET /market-time/sessions",
        Route::ok(
            r#"{"data": {"items": [
                    {"instrument-collection": "Equity",
                     "open-at": "2026-08-03T09:30:00.000-04:00"}]},
                "context": "/market-time/sessions"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let sessions = client
        .market_sessions(
            &SessionRange::between(day(2026, 8, 1), day(2026, 8, 31))
                .with_instrument_collection(InstrumentCollection::Equity),
        )
        .await
        .expect("the sessions must decode");

    assert_eq!(
        last_target(&venue),
        "/market-time/sessions?to-date=2026-08-31&from-date=2026-08-01\
         &instrument-collection=Equity"
    );
    assert_eq!(sessions.len(), 1);

    let _ = client
        .market_sessions(&SessionRange::until(day(2026, 8, 31)))
        .await;
    assert_eq!(
        last_target(&venue),
        "/market-time/sessions?to-date=2026-08-31"
    );
}

/// The futures family is keyed by collection in the **path**, which therefore
/// goes through the shared encoder.
#[tokio::test]
async fn the_futures_family_puts_its_collection_in_the_path() {
    let venue = venue_with(vec![(
        "GET /market-time/futures/holidays/CME",
        Route::ok(
            r#"{"data": {"market-holidays": ["2026-01-01"],
                         "market-half-days": ["2026-11-27"]},
                "context": "/market-time/futures/holidays/CME"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let calendar = client
        .futures_holidays(&InstrumentCollection::Cme)
        .await
        .expect("the calendar must decode");

    assert_eq!(last_target(&venue), "/market-time/futures/holidays/CME");
    assert!(calendar.is_holiday(day(2026, 1, 1)));
    assert!(calendar.is_half_day(day(2026, 11, 27)));
}

/// Omitting `date` omits the key, which leaves the venue's "relative to now"
/// default in place rather than substituting this machine's today.
#[tokio::test]
async fn omitting_the_date_omits_the_parameter() {
    let venue = venue_with(vec![(
        "GET /market-time/equities/sessions/next",
        Route::ok(
            r#"{"data": {"session-date": "2026-08-04"},
                "context": "/market-time/equities/sessions/next"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let next = client
        .next_equities_session(None)
        .await
        .expect("the session must decode");
    assert_eq!(last_target(&venue), "/market-time/equities/sessions/next");
    assert_eq!(next.session_date, Some(day(2026, 8, 4)));

    let _ = client.next_equities_session(Some(day(2026, 8, 10))).await;
    assert_eq!(
        last_target(&venue),
        "/market-time/equities/sessions/next?date=2026-08-10"
    );
}
