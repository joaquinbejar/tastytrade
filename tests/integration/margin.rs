//! Margin and risk parameters, end to end.
//!
//! Two things a unit test cannot reach: that `estimate_margin` refuses a bad
//! request **before anything is sent**, and that `span_rows` puts both of its
//! required parameters on the wire and can read past page one.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;

use crate::support::{MockVenue, Route, one_account_body, sentinel, token_response_body};

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

async fn venue_with(extra: Vec<(String, Route)>) -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(one_account_body(sentinel::ACCOUNT_NUMBER)),
    );
    for (key, route) in extra {
        routes.insert(key, route);
    }
    MockVenue::start(routes).await
}

fn leg() -> MarginOrderLeg {
    MarginOrderLeg {
        symbol: "AAPL".to_string(),
        instrument_type: InstrumentType::Equity,
        quantity: Decimal::from(1),
        action: Action::BuyToOpen,
        remaining_quantity: None,
    }
}

fn request(legs: Vec<MarginOrderLeg>) -> MarginOrderRequest {
    // A `Limit` needs a working price, the same rule placement holds an order
    // to. Estimating a limit order with no price estimates a different order.
    MarginOrderRequest::new(
        sentinel::ACCOUNT_NUMBER,
        "AAPL",
        InstrumentType::Equity,
        OrderType::Limit,
        TimeInForce::Day,
        legs,
    )
    .with_price(Decimal::from(100), PriceEffect::Debit)
}

/// The venue's own requirements payload, through the real transport.
#[tokio::test]
async fn the_requirements_report_arrives_with_its_nesting() {
    let venue = venue_with(vec![(
        format!(
            "GET /margin/accounts/{}/requirements",
            sentinel::ACCOUNT_NUMBER
        ),
        Route::ok(include_str!("../../Doc/margin_requirements.json")),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let report = accounts[0]
        .margin_requirements()
        .await
        .expect("the report must decode");

    assert_eq!(report.groups.len(), 2);
    assert!(!report.groups[0].groups.is_empty());
}

/// A local refusal sends nothing. Asserted by the absence of the request, not
/// only by the error — an error that still routed would be a different bug
/// with the same message, and this one is on a margin path.
#[tokio::test]
async fn an_invalid_margin_request_never_reaches_the_venue() {
    let venue = venue_with(vec![]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    for (what, bad) in [
        ("no legs", request(vec![])),
        (
            "five legs",
            request((0..5).map(|_| leg()).collect::<Vec<_>>()),
        ),
        ("duplicate legs", request(vec![leg(), leg()])),
    ] {
        let error = accounts[0].estimate_margin(&bad).await.expect_err(what);
        assert!(!error.is_retryable(), "{what} must not be retryable");
    }

    assert!(
        venue
            .requests()
            .iter()
            .all(|request| !request.target.contains("/margin/")),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// The body names an account and so does the path. If they disagree the venue
/// has to pick one, and which one it picks is not something to discover on a
/// figure somebody sizes a position from.
#[tokio::test]
async fn a_request_naming_another_account_is_refused() {
    let venue = venue_with(vec![]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let elsewhere = MarginOrderRequest::new(
        "SOMEONE-ELSE",
        "AAPL",
        InstrumentType::Equity,
        OrderType::Limit,
        TimeInForce::Day,
        vec![leg()],
    );

    let error = accounts[0]
        .estimate_margin(&elsewhere)
        .await
        .expect_err("the account in the body must match the path");

    assert!(format!("{error}").contains("different account"), "{error}");
}

/// A valid estimate reaches the venue with the fields the order type does not
/// carry, and decodes.
#[tokio::test]
async fn a_valid_estimate_sends_the_required_fields() {
    let venue = venue_with(vec![(
        format!("POST /margin/accounts/{}/dry-run", sentinel::ACCOUNT_NUMBER),
        Route::ok(include_str!("../../Doc/margin_dry_run.json")),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let estimate = accounts[0]
        .estimate_margin(&request(vec![leg()]))
        .await
        .expect("the estimate must decode");

    assert_eq!(estimate.is_spread, Some(false));

    let sent = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.contains("/margin/"))
        .expect("the request must have been sent");
    let body: serde_json::Value = serde_json::from_str(&sent.body).expect("a JSON body");

    assert_eq!(body["account-number"], sentinel::ACCOUNT_NUMBER);
    assert_eq!(body["underlying-symbol"], "AAPL");
    assert_eq!(body["underlying-instrument-type"], "Equity");
}

/// Both required parameters reach the wire, and the second page is reachable.
#[tokio::test]
async fn span_rows_sends_both_required_parameters_and_pages() {
    let page = |offset: usize, total: usize| {
        serde_json::json!({
            "data": {"items": [{
                "exchange": "CME",
                "file-date": "2026-08-03",
                "row-index": offset,
                "row-data": "a fixed-width record"
            }]},
            "pagination": {
                "per-page": 1, "page-offset": offset, "item-offset": offset,
                "total-items": total, "total-pages": total, "current-item-count": 1,
                "previous-link": null, "next-link": null, "paging-link-template": null
            },
            "context": "/span/rows"
        })
        .to_string()
    };

    let venue = venue_with(vec![("GET /span/rows".to_string(), Route::ok(page(1, 3)))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let rows = client
        .span_rows(
            NaiveDate::from_ymd_opt(2026, 8, 3).expect("a real date"),
            SpanExchange::Cme,
            &PageRequest::first().next_page().with_per_page(1),
        )
        .await
        .expect("the page must decode");

    let target = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/span/rows"))
        .expect("the request must have been sent")
        .target;
    assert_eq!(
        target,
        "/span/rows?date=2026-08-03&exchange=CME&page-offset=1&per-page=1"
    );
    assert_eq!(rows.pagination.page_offset, 1);
    assert!(rows.has_more());
    assert_eq!(
        rows.items[0].row_data.as_deref(),
        Some("a fixed-width record")
    );
}

/// The public configuration goes through the same authenticated client as
/// everything else.
#[tokio::test]
async fn the_public_margin_configuration_decodes() {
    let venue = venue_with(vec![(
        "GET /margin-requirements-public-configuration".to_string(),
        Route::ok(
            r#"{"data": {"risk-free-rate": "0.0525"},
                "context": "/margin-requirements-public-configuration"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let configuration = client
        .margin_requirements_configuration()
        .await
        .expect("the configuration must decode");

    assert_eq!(
        configuration.risk_free_rate.expect("a rate").to_string(),
        "0.0525"
    );
}

/// The underlying is a path segment, so a separator in it must be encoded.
#[tokio::test]
async fn the_effective_requirement_encodes_its_underlying() {
    let venue = venue_with(vec![(
        format!(
            "GET /accounts/{}/margin-requirements/BRK%2FB/effective",
            sentinel::ACCOUNT_NUMBER
        ),
        Route::ok(
            r#"{"data": {"underlying-symbol": "BRK/B", "long-equity-initial": "0.5"},
                "context": "/effective"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let requirement = accounts[0]
        .effective_margin_requirement("BRK/B")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(requirement.underlying_symbol.as_deref(), Some("BRK/B"));
    assert_eq!(
        requirement
            .long_equity_initial
            .expect("a ratio")
            .to_string(),
        "0.5"
    );
}
