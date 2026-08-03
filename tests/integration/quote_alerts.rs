//! Quote alerts, end to end.
//!
//! The property worth pinning: the type the REST half returns is the same type
//! the account streamer delivers when an alert fires. If those two ever drift,
//! a caller sets an alert with one shape and receives it as another.

use std::collections::HashMap;

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

const ALERT: &str = r#"{"alert-external-id": "alert-1", "symbol": "AAPL",
                        "field": "Last", "operator": ">", "threshold": "200.00",
                        "threshold-numeric": 200.0,
                        "created-at": "2026-08-01T12:00:00.000+00:00"}"#;

#[tokio::test]
async fn the_listing_decodes_into_the_streaming_type() {
    let venue = venue_with(vec![(
        "GET /quote-alerts",
        Route::ok(format!(
            r#"{{"data": {{"items": [{ALERT}]}}, "context": "/quote-alerts"}}"#
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let alerts = client.quote_alerts().await.expect("the alerts must decode");

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].field, Some(QuoteAlertField::Last));
    assert_eq!(alerts[0].operator, Some(QuoteAlertOperator::Above));
    assert_eq!(
        alerts[0]
            .threshold_numeric
            .expect("a threshold")
            .to_string(),
        "200.0"
    );
}

/// The body carries the four fields the venue marks required and omits the
/// rest, rather than sending them as null.
#[tokio::test]
async fn creating_an_alert_sends_the_required_fields() {
    let venue = venue_with(vec![(
        "POST /quote-alerts",
        Route::ok(format!(
            r#"{{"data": {ALERT}, "context": "/quote-alerts"}}"#
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let created = client
        .create_quote_alert(&NewQuoteAlert::new(
            "AAPL",
            QuoteAlertField::Last,
            QuoteAlertOperator::Above,
            Decimal::new(20000, 2),
        ))
        .await
        .expect("the alert must be created");

    assert_eq!(created.alert_external_id.as_deref(), Some("alert-1"));

    let sent = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target == "/quote-alerts")
        .expect("the create must have been sent");
    let body: serde_json::Value = serde_json::from_str(&sent.body).expect("a JSON body");

    assert_eq!(body["symbol"], "AAPL");
    assert_eq!(body["field"], "Last");
    assert_eq!(body["operator"], ">");
    assert_eq!(body["threshold"], "200.00");
    assert!(body.get("dx-symbol").is_none(), "{body}");
}

/// A local refusal sends nothing.
#[tokio::test]
async fn a_zero_threshold_never_reaches_the_venue() {
    let venue = venue_with(vec![(
        "POST /quote-alerts",
        Route::ok(format!(
            r#"{{"data": {ALERT}, "context": "/quote-alerts"}}"#
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .create_quote_alert(&NewQuoteAlert::new(
            "AAPL",
            QuoteAlertField::Bid,
            QuoteAlertOperator::Below,
            Decimal::ZERO,
        ))
        .await
        .expect_err("a zero threshold fires immediately");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.method != "POST" || request.target == "/oauth/token"),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// The identifier is a path segment like any other.
#[tokio::test]
async fn cancelling_encodes_the_identifier() {
    let venue = venue_with(vec![(
        "DELETE /quote-alerts/alert%2F1",
        Route::ok(format!(
            r#"{{"data": {ALERT}, "context": "/quote-alerts"}}"#
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    client
        .cancel_quote_alert("alert/1")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(
        venue
            .requests()
            .into_iter()
            .rfind(|request| request.target.starts_with("/quote-alerts"))
            .expect("the request must have been sent")
            .target,
        "/quote-alerts/alert%2F1"
    );
}

/// The venue answers `204 No Content`, and a successful cancellation must not
/// be reported as a failure.
///
/// Asking the generic verb for a `QuoteAlert` back made this call cancel the
/// alert and then fail decoding the empty body, so the caller was handed an
/// error for a mutation that had already happened — the worst answer available
/// about a state change.
#[tokio::test]
async fn a_cancellation_succeeds_on_an_empty_response() {
    let venue = venue_with(vec![("DELETE /quote-alerts/abc", Route::status(204, ""))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    client
        .cancel_quote_alert("abc")
        .await
        .expect("204 with no body is a successful cancellation");

    // A failure status still reports the broker's document rather than being
    // swallowed along with the empty-body case.
    let venue = venue_with(vec![(
        "DELETE /quote-alerts/gone",
        Route::status(
            404,
            r#"{"error":{"code":"not_found","message":"no such alert"}}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .cancel_quote_alert("gone")
        .await
        .expect_err("a 404 is still a failure");
    let rendered = format!("{error}");
    assert!(rendered.contains("404"), "{rendered}");
    assert!(rendered.contains("no such alert"), "{rendered}");
}
