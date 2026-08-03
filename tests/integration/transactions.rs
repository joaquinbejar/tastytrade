//! The account ledger, end to end against the loopback venue.
//!
//! Pagination is the point: a ledger is the one listing where reading past the
//! first page is not optional — the venue's own example reports 1,622 items
//! across seven pages, and a P&L built from page one is wrong.

use std::collections::HashMap;

use chrono::NaiveDate;
use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;

use crate::support::{MockVenue, Route, one_account_body, sentinel, token_response_body};

/// The venue's own listing payload, account number redacted.
const LISTING: &str = include_str!("../../Doc/transactions_listing.json");

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

fn account_path(suffix: &str) -> String {
    format!("GET /accounts/{}{suffix}", sentinel::ACCOUNT_NUMBER)
}

fn last_query(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.contains("/transactions"))
        .expect("a transactions request must have been sent")
        .target
        .split_once('?')
        .map(|(_, query)| query.to_string())
        .unwrap_or_default()
}

/// A listing body reporting `page_offset` of `total_pages`, carrying the
/// venue's own three rows.
fn listing_on_page(page_offset: usize, total_pages: usize) -> String {
    let body: serde_json::Value = serde_json::from_str(LISTING).expect("the fixture is JSON");
    let items = body["data"]["items"].clone();

    serde_json::json!({
        "data": { "items": items },
        "pagination": {
            "per-page": 3,
            "page-offset": page_offset,
            "item-offset": page_offset * 3,
            "total-items": total_pages * 3,
            "total-pages": total_pages,
            "current-item-count": 3,
            "previous-link": null,
            "next-link": null,
            "paging-link-template": null
        },
        "context": "/accounts/x/transactions"
    })
    .to_string()
}

/// The venue's own payload decodes into the model, envelope and all.
#[tokio::test]
async fn the_venues_own_listing_decodes() {
    let venue = venue_with(vec![(
        account_path("/transactions"),
        Route::ok(listing_on_page(0, 1)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let page = accounts[0]
        .transactions(&TransactionFilter::new())
        .await
        .expect("the listing must decode");

    assert_eq!(page.len(), 3);
    assert_eq!(
        page.items[0].transaction_sub_type,
        Some(TransactionSubType::Dividend)
    );
    // A dividend row and a cash row in one page: the second has no quantity,
    // which is what a struct of required fields would have rejected.
    assert!(page.items[0].quantity.is_some());
    assert!(page.items[2].quantity.is_none());
}

/// Reading past page one, which is what `Paginated<T>` exists for.
#[tokio::test]
async fn a_second_page_of_the_ledger_is_reachable() {
    let venue = venue_with(vec![(
        account_path("/transactions"),
        Route::ok(listing_on_page(1, 7)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let page = accounts[0]
        .transactions(&TransactionFilter::new().with_page(PageRequest::first().next_page()))
        .await
        .expect("the page must decode");

    assert_eq!(last_query(&venue), "page-offset=1");
    assert_eq!(page.pagination.page_offset, 1);
    assert_eq!(page.pagination.total_pages, 7);
    assert!(page.has_more(), "page 1 of 7 is not the last one");
}

/// `type` and `types` are mutually exclusive at the venue, and only one set of
/// keys can ever reach it.
#[tokio::test]
async fn several_kinds_reach_the_venue_as_repeated_keys() {
    let venue = venue_with(vec![(
        account_path("/transactions"),
        Route::ok(listing_on_page(0, 1)),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let _ = accounts[0]
        .transactions(
            &TransactionFilter::new()
                .with_types(TransactionTypes::Several(vec![
                    TransactionType::Trade,
                    TransactionType::MoneyMovement,
                ]))
                .with_sub_types(&[TransactionSubType::Dividend])
                .with_dates(
                    Some(NaiveDate::from_ymd_opt(2026, 1, 1).expect("a real date")),
                    None,
                ),
        )
        .await;

    assert_eq!(
        last_query(&venue),
        "types%5B%5D=Trade&types%5B%5D=Money+Movement\
         &sub-type%5B%5D=Dividend&start-date=2026-01-01"
    );
}

/// Omitting the date must omit the key, so the venue's documented "today"
/// default survives. Sending this process's idea of today would substitute the
/// wrong clock.
#[tokio::test]
async fn total_fees_omits_the_date_when_none_is_given() {
    let venue = venue_with(vec![(
        account_path("/transactions/total-fees"),
        Route::ok(
            r#"{"data": {"total-fees": "100.0", "total-fees-effect": "Debit"},
                "context": "/accounts/x/transactions/total-fees"}"#,
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let fees = accounts[0]
        .total_fees(None)
        .await
        .expect("the venue's own payload must decode");

    assert_eq!(last_query(&venue), "");
    assert_eq!(fees.total_fees.expect("a total").to_string(), "100.0");
    assert_eq!(fees.total_fees_effect, Some(PriceEffect::Debit));

    // …and an explicit date is sent.
    let _ = accounts[0]
        .total_fees(Some(
            NaiveDate::from_ymd_opt(2026, 1, 1).expect("a real date"),
        ))
        .await;
    assert_eq!(last_query(&venue), "date=2026-01-01");
}

/// One transaction by id, from its own route.
#[tokio::test]
async fn a_single_transaction_is_fetched_by_id() {
    let body: serde_json::Value = serde_json::from_str(LISTING).expect("the fixture is JSON");
    let row = body["data"]["items"][0].clone();
    let venue = venue_with(vec![(
        account_path("/transactions/252640963"),
        Route::ok(
            serde_json::json!({"data": row, "context": "/accounts/x/transactions/252640963"})
                .to_string(),
        ),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let transaction = accounts[0]
        .transaction(252640963)
        .await
        .expect("the transaction must decode");

    assert_eq!(transaction.id, 252640963);
    assert_eq!(transaction.action, Some(TransactionAction::BuyToOpen));
}
