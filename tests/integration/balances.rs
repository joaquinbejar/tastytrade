//! Balances, snapshots and positions, end to end against the loopback venue.
//!
//! The envelope is the point here. `GET /balances` answers with `items` and
//! has since 2024-05-01; this crate decoded it as a single object, so the call
//! could only ever fail. A unit test could not have caught that — the type was
//! right, the envelope around it was not.

use std::collections::HashMap;

use chrono::NaiveDate;
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

/// A balance row in `currency`, built from the fixture the streaming tests
/// already use.
///
/// Reused rather than hand-written because `Balance` has 31 required fields
/// and a fixture that omits one fails as "nothing in the listing decoded",
/// which looks exactly like the envelope bug this file is about. One curated
/// fixture, two consumers.
fn balance_row(currency: &str, cash: &str) -> String {
    const FIXTURE: &str = include_str!("../../Doc/frames/account/account-balance.derived.json");

    let frame: serde_json::Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let mut data = frame
        .get("data")
        .cloned()
        .expect("the fixture carries a data object");

    let object = data
        .as_object_mut()
        .expect("the balance payload is an object");
    object.insert(
        "currency".to_string(),
        serde_json::Value::String(currency.to_string()),
    );
    object.insert(
        "cash-balance".to_string(),
        serde_json::Value::String(cash.to_string()),
    );

    data.to_string()
}

fn items_body(items: &[String]) -> String {
    format!(
        r#"{{"data": {{"items": [{}]}}, "context": "/accounts/x/balances"}}"#,
        items.join(",")
    )
}

/// A venue that authenticates, lists one account, and answers `extra`.
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

/// The encoded account path, since the account number carries a sentinel that
/// is not otherwise path-safe to assume.
fn account_path(suffix: &str) -> String {
    format!("/accounts/{}{suffix}", sentinel::ACCOUNT_NUMBER)
}

async fn account_on(venue: &MockVenue) -> (TastyTrade, String) {
    let client = TastyTrade::connect(&config_for(venue))
        .await
        .expect("authentication must succeed");
    (client, sentinel::ACCOUNT_NUMBER.to_string())
}

fn last_query(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.contains("/accounts/"))
        .expect("an account-scoped request must have been sent")
        .target
        .split_once('?')
        .map(|(_, query)| query.to_string())
        .unwrap_or_default()
}

/// The bug. `/balances` returns `{"data": {"items": [...]}}` and this crate
/// decoded `data` straight into a `Balance`, so the call failed against the
/// contract the venue has published since 2024-05-01.
#[tokio::test]
async fn the_balances_envelope_is_a_list_and_decodes_as_one() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balances")),
        Route::ok(items_body(&[balance_row("USD", "1234.56")])),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let rows = accounts[0]
        .balances()
        .await
        .expect("the items envelope must decode");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].currency.as_deref(), Some("USD"));
    assert_eq!(rows[0].cash_balance.to_string(), "1234.56");
}

/// One row means there is a single balance, so `balance()` still works.
#[tokio::test]
async fn a_single_row_is_still_reachable_as_the_balance() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balances")),
        Route::ok(items_body(&[balance_row("USD", "10.00")])),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let balance = accounts[0].balance().await.expect("exactly one row");

    assert_eq!(balance.currency.as_deref(), Some("USD"));
}

/// Two rows means "the balance" is ambiguous, and picking one for the caller
/// would be answering a question about money on their behalf.
#[tokio::test]
async fn several_currency_rows_refuse_to_collapse_into_one_balance() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balances")),
        Route::ok(items_body(&[
            balance_row("USD", "10.00"),
            balance_row("EUR", "20.00"),
        ])),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let error = accounts[0]
        .balance()
        .await
        .expect_err("two rows is not one balance");

    let rendered = format!("{error}");
    assert!(
        rendered.contains("USD") && rendered.contains("EUR"),
        "{rendered}"
    );
    assert!(rendered.contains("balances()"), "{rendered}");
    // The request succeeded; the answer does not fit the question.
    assert!(!error.is_retryable(), "retrying returns the same two rows");
    // Currency codes are schema. The amounts are not, and an error travels.
    assert!(
        !rendered.contains("10.00") && !rendered.contains("20.00"),
        "a balance reached an error message: {rendered}"
    );

    // …and both rows are there for a caller who asks for them.
    assert_eq!(accounts[0].balances().await.expect("two rows").len(), 2);
}

/// The per-currency endpoint, which did not exist.
#[tokio::test]
async fn a_currency_balance_is_fetched_from_its_own_route() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balances/EUR")),
        Route::ok(format!(
            r#"{{"data": {}, "context": "/accounts/x/balances/EUR"}}"#,
            balance_row("EUR", "99.99")
        )),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let balance = accounts[0]
        .balance_in("EUR")
        .await
        .expect("the currency route must answer");

    assert_eq!(balance.currency.as_deref(), Some("EUR"));
    assert_eq!(balance.cash_balance.to_string(), "99.99");
}

/// The required parameter, with the venue's spelling. It went out as `Eod`
/// before, taken from a `Display` that was really the derived `Debug`.
#[tokio::test]
async fn the_snapshot_query_sends_the_time_of_day_the_venue_uses() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balance-snapshots")),
        Route::ok(
            r#"{"data": {"items": []},
                "pagination": {"per-page": 10, "page-offset": 0, "item-offset": 0,
                               "total-items": 0, "total-pages": 1, "current-item-count": 0,
                               "previous-link": null, "next-link": null,
                               "paging-link-template": null},
                "context": "/accounts/x/balance-snapshots"}"#,
        ),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0]
        .balance_snapshots(
            &BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
                .with_currency("USD")
                .with_range(SnapshotRange::between(
                    NaiveDate::from_ymd_opt(2026, 1, 1).expect("a real date"),
                    NaiveDate::from_ymd_opt(2026, 1, 31).expect("a real date"),
                ))
                .with_page(PageRequest::first().with_per_page(10)),
        )
        .await;

    assert_eq!(
        last_query(&venue),
        "page-offset=0&per-page=10&time-of-day=EOD&currency=USD\
         &start-date=2026-01-01&end-date=2026-01-31"
    );
}

/// A single day and a range are alternatives at the type level, so the query
/// can only ever carry one of them.
#[tokio::test]
async fn a_snapshot_date_excludes_the_range_keys() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/balance-snapshots")),
        Route::status(404, "{}"),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0]
        .balance_snapshots(
            &BalanceSnapshotFilter::at(SnapshotTimeOfDay::Bod).with_range(SnapshotRange::on(
                NaiveDate::from_ymd_opt(2026, 3, 14).expect("a real date"),
            )),
        )
        .await;

    assert_eq!(
        last_query(&venue),
        "time-of-day=BOD&snapshot-date=2026-03-14"
    );
}

/// Every position filter, with the arrays as repeated keys.
#[tokio::test]
async fn the_position_filters_reach_the_venue() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/positions")),
        Route::ok(items_body(&[])),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0]
        .positions_matching(
            &PositionFilter::new()
                .with_closed_positions(true)
                .with_marks(true)
                .with_instrument_type(InstrumentType::EquityOption)
                .with_underlying_symbols(&["AAPL", "SPY"])
                .with_partition_keys(&["main"]),
        )
        .await;

    assert_eq!(
        last_query(&venue),
        "include-closed-positions=true&include-marks=true\
         &instrument-type=Equity+Option\
         &partition-keys%5B%5D=main\
         &underlying-symbol%5B%5D=AAPL&underlying-symbol%5B%5D=SPY"
    );
}

/// The unfiltered call must send nothing, or every existing caller silently
/// starts making a different request.
#[tokio::test]
async fn an_unfiltered_position_call_sends_no_query_at_all() {
    let venue = venue_with(vec![(
        format!("GET {}", account_path("/positions")),
        Route::ok(items_body(&[])),
    )])
    .await;
    let (client, _) = account_on(&venue).await;
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0].positions().await;

    assert_eq!(last_query(&venue), "");
}
