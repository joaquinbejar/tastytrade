//! Complex orders, end to end against the loopback venue.
//!
//! The reviewed path is the point: a complex order routes real money, so it
//! cannot be placed without a receipt from a dry run of that same container,
//! against that same account, on that same deployment.

use std::collections::HashMap;

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

fn route(verb: &str, suffix: &str) -> String {
    format!("{verb} /accounts/{}{suffix}", sentinel::ACCOUNT_NUMBER)
}

fn container_body() -> String {
    format!(
        r#"{{"data": {{"id": "abc-123", "account-number": "{}", "type": "OCO",
                       "orders": [{{"id": "1", "status": "Live", "complex-order-tag": "entry"}},
                                  {{"id": "2", "status": "Live"}}],
                       "related-orders": []}},
            "context": "/complex-orders"}}"#,
        sentinel::ACCOUNT_NUMBER
    )
}

fn dry_run_body() -> String {
    include_str!("../../Doc/order_dry_run.json").to_string()
}

fn component(price: &str) -> Order {
    OrderBuilder::default()
        .time_in_force(TimeInForce::Gtc)
        .order_type(OrderType::Limit)
        .price(Decimal::from_str_exact(price).expect("a price"))
        .price_effect(PriceEffect::Credit)
        .legs(vec![
            OrderLegBuilder::default()
                .instrument_type(InstrumentType::Equity)
                .symbol("AAPL")
                .quantity(Decimal::ONE)
                .action(Action::SellToClose)
                .build()
                .expect("a valid leg"),
        ])
        .build()
        .expect("a valid order")
}

/// OCO means "one cancels other". A one-sided one never reaches the venue.
#[tokio::test]
async fn a_one_sided_oco_never_reaches_the_venue() {
    let venue = venue_with(vec![(
        route("POST", "/complex-orders/dry-run"),
        Route::ok(dry_run_body()),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let error = accounts[0]
        .review_complex_order(&ComplexOrderRequest::new(
            ComplexOrderType::Oco,
            vec![component("2.00")],
        ))
        .await
        .expect_err("OCO needs two components");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| !request.target.contains("complex-orders")),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// The whole reviewed path, and the container that comes back.
#[tokio::test]
async fn a_reviewed_oco_is_placed_and_decodes() {
    let venue = venue_with(vec![
        (
            route("POST", "/complex-orders/dry-run"),
            Route::ok(dry_run_body()),
        ),
        (
            route("POST", "/complex-orders"),
            Route::ok(container_body()),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let receipt = accounts[0]
        .review_complex_order(&ComplexOrderRequest::new(
            ComplexOrderType::Oco,
            vec![component("2.00"), component("0.50")],
        ))
        .await
        .expect("the dry run must succeed");
    assert!(receipt.is_clean());

    let placed = accounts[0]
        .place_reviewed_complex_order(receipt.accept().expect("a clean dry run"))
        .await
        .expect("the container must decode");

    assert_eq!(placed.id.map(|id| id.0), Some("abc-123".to_string()));
    assert_eq!(placed.complex_order_type, Some(ComplexOrderType::Oco));
    assert_eq!(placed.orders.len(), 2);
    assert_eq!(placed.orders[0].complex_order_tag.as_deref(), Some("entry"));

    // The dry-run body carries the strategy and its components.
    let sent = venue
        .requests()
        .into_iter()
        .find(|request| request.target.ends_with("/complex-orders/dry-run"))
        .expect("the dry run must have been sent");
    let body: serde_json::Value = serde_json::from_str(&sent.body).expect("a JSON body");
    assert_eq!(body["type"], "OCO");
    assert_eq!(body["orders"].as_array().map(Vec::len), Some(2));
    // A non-PAIRS container sends no threshold at all, rather than null.
    assert!(body.get("ratio-price-threshold").is_none(), "{body}");
}

/// A receipt from one deployment must not authorise a placement on another.
#[tokio::test]
async fn a_receipt_from_another_venue_is_refused() {
    let first = venue_with(vec![(
        route("POST", "/complex-orders/dry-run"),
        Route::ok(dry_run_body()),
    )])
    .await;
    let second = venue_with(vec![(
        route("POST", "/complex-orders"),
        Route::ok(container_body()),
    )])
    .await;

    let reviewer = TastyTrade::connect(&config_for(&first))
        .await
        .expect("authentication must succeed");
    let placer = TastyTrade::connect(&config_for(&second))
        .await
        .expect("authentication must succeed");

    let reviewed = reviewer.accounts().await.expect("one account")[0]
        .review_complex_order(&ComplexOrderRequest::new(
            ComplexOrderType::Oco,
            vec![component("2.00"), component("0.50")],
        ))
        .await
        .expect("the dry run must succeed")
        .accept()
        .expect("a clean dry run");

    let elsewhere = placer.accounts().await.expect("one account");
    let error = elsewhere[0]
        .place_reviewed_complex_order(reviewed)
        .await
        .expect_err("a receipt from another deployment must not be honoured");

    assert!(format!("{error}").contains("different venue"), "{error}");
    assert!(
        second
            .requests()
            .iter()
            .all(|request| !request.target.ends_with("/complex-orders")),
        "nothing may have been placed on the second venue"
    );
}

/// The PAIRS threshold change is its own narrow route and its own narrow type.
#[tokio::test]
async fn a_pairs_threshold_change_patches_only_the_threshold() {
    let venue = venue_with(vec![
        (
            route("POST", "/complex-orders/abc-123/dry-run"),
            Route::ok(dry_run_body()),
        ),
        (
            route("PATCH", "/complex-orders/abc-123"),
            Route::ok(container_body()),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let id = ComplexOrderId::from("abc-123");
    let receipt = accounts[0]
        .review_pairs_threshold(
            &id,
            &PairsThresholdEdit::new(RatioPriceComparator::LessOrEqual, Decimal::new(125, 2)),
        )
        .await
        .expect("the dry run must succeed");

    let _ = accounts[0]
        .place_reviewed_pairs_threshold(receipt.accept().expect("a clean dry run"))
        .await;

    let patched = venue
        .requests()
        .into_iter()
        .rfind(|request| request.method == "PATCH")
        .expect("a PATCH must have been sent");
    let body: serde_json::Value = serde_json::from_str(&patched.body).expect("a JSON body");

    assert_eq!(body["ratio-price-comparator"], "lte");
    assert_eq!(body.as_object().map(serde_json::Map::len), Some(2));
}

/// The identifier is a path segment and a string, so it gets encoded like any
/// other.
#[tokio::test]
async fn a_complex_order_id_is_encoded_into_the_path() {
    let venue = venue_with(vec![(
        route("GET", "/complex-orders/abc%2F123"),
        Route::ok(container_body()),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let container = accounts[0]
        .complex_order(&ComplexOrderId::from("abc/123"))
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(container.orders.len(), 2);
}
