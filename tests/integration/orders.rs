//! The order lifecycle, end to end against the loopback venue.
//!
//! What matters here is the reviewed path: a replacement cannot be applied
//! without a receipt from a dry-run of that same order against that same
//! account **and** that same deployment, and the verb it goes out with is the
//! one recorded at review time.

use std::collections::HashMap;

use rust_decimal::Decimal;
use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;

use tracing::Level;

use crate::support::{
    MockVenue, Route, capture_logs_at, one_account_body, sentinel, token_response_body,
};

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

fn account_route(verb: &str, suffix: &str) -> String {
    format!("{verb} /accounts/{}{suffix}", sentinel::ACCOUNT_NUMBER)
}

/// One order record, complete enough to decode.
fn order_body(id: u64, status: &str) -> String {
    format!(
        r#"{{
            "id": {id},
            "account-number": "{}",
            "time-in-force": "Day",
            "order-type": "Limit",
            "size": "1",
            "underlying-symbol": "AAPL",
            "price": "1.00",
            "price-effect": "Debit",
            "status": "{status}",
            "cancellable": true,
            "editable": true,
            "edited": false,
            "legs": []
        }}"#,
        sentinel::ACCOUNT_NUMBER
    )
}

fn paginated(items: &[String]) -> String {
    format!(
        r#"{{
            "data": {{"items": [{}]}},
            "pagination": {{"per-page": 25, "page-offset": 0, "item-offset": 0,
                            "total-items": {}, "total-pages": 1,
                            "current-item-count": {},
                            "previous-link": null, "next-link": null,
                            "paging-link-template": null}},
            "context": "/accounts/x/orders"
        }}"#,
        items.join(","),
        items.len(),
        items.len()
    )
}

/// The venue's own dry-run payload, account number redacted.
///
/// Reused rather than hand-written: `DryRunResult` requires a whole order
/// record, a buying-power effect and a fee calculation, and a fixture that
/// omits one fails in a way that looks like the reviewed path is broken.
fn dry_run_body() -> String {
    include_str!("../../Doc/order_dry_run.json").to_string()
}

fn amendment() -> OrderAmendment {
    OrderAmendment::new(
        OrderType::Limit,
        TimeInForce::Day,
        Decimal::ZERO,
        PriceEffect::Debit,
        PriceEffect::Debit,
    )
    .with_price(Decimal::ONE)
}

fn last_target(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.contains("/orders"))
        .expect("an order request must have been sent")
        .target
}

/// History statuses are repeated keys; the live endpoint takes a single one.
/// Getting that backwards means the live listing comes back unfiltered and
/// looks filtered.
#[tokio::test]
async fn history_and_live_filters_spell_status_differently() {
    let venue = venue_with(vec![
        (account_route("GET", "/orders"), Route::ok(paginated(&[]))),
        (
            account_route("GET", "/orders/live"),
            Route::ok(paginated(&[])),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0]
        .search_orders(
            &OrderFilter::new().with_statuses(&[OrderStatus::Live, OrderStatus::CancelRequested]),
        )
        .await;
    assert_eq!(
        last_target(&venue).split_once('?').map(|(_, q)| q),
        Some("status%5B%5D=Live&status%5B%5D=Cancel+Requested")
    );

    let _ = accounts[0]
        .live_orders_matching(&LiveOrderFilter::new().with_status(OrderStatus::Live))
        .await;
    assert_eq!(
        last_target(&venue).split_once('?').map(|(_, q)| q),
        Some("status=Live")
    );
}

/// The no-argument call must keep working and keep sending nothing.
#[tokio::test]
async fn the_unfiltered_live_call_is_unchanged() {
    let venue = venue_with(vec![(
        account_route("GET", "/orders/live"),
        Route::ok(paginated(&[order_body(1, "Live")])),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let orders = accounts[0].live_orders().await.expect("one order");

    assert_eq!(orders.len(), 1);
    assert!(!last_target(&venue).contains('?'));
}

/// A status the venue adds later must not make the order disappear. `Items<T>`
/// skips what it cannot parse, so a strict enum here loses an order silently —
/// the worst failure available on a live-orders listing.
#[tokio::test]
async fn an_unrecognised_status_keeps_the_order_in_the_listing() {
    let venue = venue_with(vec![(
        account_route("GET", "/orders/live"),
        Route::ok(paginated(&[order_body(7, "Something New")])),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let orders = accounts[0].live_orders().await.expect("the order survives");

    assert_eq!(orders.len(), 1, "the order must not vanish");
    assert_eq!(orders[0].status.as_wire(), "Something New");
    assert!(!orders[0].status.is_known());
    // …and an unrecognised status is not assumed to be finished.
    assert!(!orders[0].status.is_terminal());
}

/// One order by id, from its own route.
#[tokio::test]
async fn a_single_order_is_fetched_by_id() {
    let venue = venue_with(vec![(
        account_route("GET", "/orders/42"),
        Route::ok(format!(
            r#"{{"data": {}, "context": "/orders/42"}}"#,
            order_body(42, "Live")
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let order = accounts[0]
        .order(OrderId(42))
        .await
        .expect("the order must decode");

    assert_eq!(order.id.0, 42);
    assert_eq!(order.status, OrderStatus::Live);
}

/// The verb is decided by what the receipt records, not by which method was
/// called afterwards.
#[tokio::test]
async fn the_intent_recorded_at_review_time_chooses_the_verb() {
    let venue = venue_with(vec![
        (
            account_route("POST", "/orders/42/dry-run"),
            Route::ok(dry_run_body()),
        ),
        (
            account_route("PUT", "/orders/42"),
            Route::ok(format!(
                r#"{{"data": {}, "context": "/orders/42"}}"#,
                order_body(42, "Live")
            )),
        ),
        (
            account_route("PATCH", "/orders/42"),
            Route::ok(format!(
                r#"{{"data": {}, "context": "/orders/42"}}"#,
                order_body(42, "Live")
            )),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    for (intent, expected) in [
        (AmendmentIntent::Replace, "PUT"),
        (AmendmentIntent::Edit, "PATCH"),
    ] {
        let receipt = accounts[0]
            .review_amendment(OrderId(42), intent, &amendment())
            .await
            .expect("the dry run must succeed");
        assert_eq!(receipt.intent(), intent);

        let _ = accounts[0]
            .place_reviewed_amendment(receipt.accept().expect("a clean dry run"))
            .await;

        let sent = venue
            .requests()
            .into_iter()
            .rfind(|request| request.method == expected)
            .unwrap_or_else(|| panic!("a {expected} must have been sent"));
        assert!(sent.target.ends_with("/orders/42"), "{}", sent.target);
    }
}

/// A receipt from one deployment must not authorise an amendment on another.
/// Certification reuses production account numbering, so the account number
/// alone is not enough.
#[tokio::test]
async fn a_receipt_from_another_venue_is_refused() {
    let first = venue_with(vec![(
        account_route("POST", "/orders/42/dry-run"),
        Route::ok(dry_run_body()),
    )])
    .await;
    let second = venue_with(vec![(
        account_route("PUT", "/orders/42"),
        Route::ok(format!(
            r#"{{"data": {}, "context": "/orders/42"}}"#,
            order_body(42, "Live")
        )),
    )])
    .await;

    let reviewer = TastyTrade::connect(&config_for(&first))
        .await
        .expect("authentication must succeed");
    let placer = TastyTrade::connect(&config_for(&second))
        .await
        .expect("authentication must succeed");

    let reviewed = reviewer.accounts().await.expect("one account")[0]
        .review_amendment(OrderId(42), AmendmentIntent::Replace, &amendment())
        .await
        .expect("the dry run must succeed")
        .accept()
        .expect("a clean dry run");

    let elsewhere = placer.accounts().await.expect("one account");
    let error = elsewhere[0]
        .place_reviewed_amendment(reviewed)
        .await
        .expect_err("a receipt from another deployment must not be honoured");

    assert!(format!("{error}").contains("different venue"), "{error}");
    assert!(
        second
            .requests()
            .iter()
            .all(|request| request.method != "PUT"),
        "nothing may have been sent to the second venue"
    );
}

/// The local checks run before the dry run, so an impossible amendment costs
/// no round trip and cannot leave a working order in an unclear state.
#[tokio::test]
async fn an_impossible_amendment_never_reaches_the_venue() {
    let venue = venue_with(vec![(
        account_route("POST", "/orders/42/dry-run"),
        Route::ok(dry_run_body()),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    // A limit order with no price.
    let priceless = OrderAmendment::new(
        OrderType::Limit,
        TimeInForce::Day,
        Decimal::ZERO,
        PriceEffect::Debit,
        PriceEffect::Debit,
    );
    let error = accounts[0]
        .review_amendment(OrderId(42), AmendmentIntent::Replace, &priceless)
        .await
        .expect_err("a limit order needs a price");
    assert!(!error.is_retryable());

    // A good-til-date expiry on an order that is not GTD.
    let mismatched = amendment()
        .with_gtc_date(chrono::NaiveDate::from_ymd_opt(2026, 12, 31).expect("a real date"));
    assert!(
        accounts[0]
            .review_amendment(OrderId(42), AmendmentIntent::Edit, &mismatched)
            .await
            .is_err()
    );

    assert!(
        venue
            .requests()
            .iter()
            .all(|request| !request.target.contains("dry-run")),
        "nothing may have been sent: {:?}",
        venue.requests()
    );
}

/// `account-numbers[]` is required, and the constructor makes an empty
/// selection unrepresentable.
#[tokio::test]
async fn a_customer_search_always_sends_at_least_one_account() {
    let venue = venue_with(vec![(
        "GET /customers/me/orders".to_string(),
        Route::ok(paginated(&[])),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let _ = client
        .customer_orders(&CustomerOrderFilter::for_accounts(
            accounts[0].number(),
            &[],
        ))
        .await;

    let target = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/customers/me/orders"))
        .expect("the request must have been sent")
        .target;
    assert!(
        target.contains("account-numbers%5B%5D="),
        "the required parameter must be sent: {target}"
    );
}

/// The customer order endpoints take account numbers as query parameters, and
/// a failure must not report them back.
///
/// This is the case path redaction could not reach: `RequestReport.operation`
/// keeps the whole URL, so a transport or HTTP failure rendered every account
/// number the filter asked about, in `Display`, in `Debug` and in the DEBUG
/// line the client writes about the request.
#[tokio::test]
async fn a_failing_customer_order_search_reports_no_account_number() {
    const ERROR: &str = r#"{"error":{"code":"oops","message":"server"}}"#;

    let venue = venue_with(vec![
        (
            "GET /customers/me/orders".to_string(),
            Route::status(500, ERROR),
        ),
        (
            "GET /customers/me/orders/live".to_string(),
            Route::status(500, ERROR),
        ),
    ])
    .await;

    let (errors, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config_for(&venue))
            .await
            .expect("authentication must succeed");
        let history = client
            .customer_orders(&CustomerOrderFilter::for_accounts(
                AccountNumber::from(sentinel::ACCOUNT_NUMBER),
                &[AccountNumber::from("5WX00002")],
            ))
            .await
            .expect_err("a 500 must surface as an error");
        let live = client
            .customer_live_orders(&CustomerLiveOrderFilter::for_accounts(
                AccountNumber::from(sentinel::ACCOUNT_NUMBER),
                &[AccountNumber::from("5WX00002")],
            ))
            .await
            .expect_err("a 500 must surface as an error");
        (history, live)
    })
    .await;

    // Both were really sent, so this is not passing because nothing happened.
    assert_eq!(
        venue
            .requests()
            .iter()
            .filter(|request| request.target.starts_with("/customers/me/orders"))
            .count(),
        2
    );

    let rendered = format!("{} {:?} {} {:?}", errors.0, errors.0, errors.1, errors.1);
    for number in [sentinel::ACCOUNT_NUMBER, "5WX00002"] {
        assert!(
            !rendered.contains(number),
            "an account number reached the error: {rendered}"
        );
        assert!(
            !logs.contents().contains(number),
            "an account number reached a log line"
        );
    }
    assert!(rendered.contains("{account}"), "{rendered}");
}
