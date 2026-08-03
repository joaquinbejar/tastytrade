//! What the instrument listings put on the wire, and what they make of the
//! answer.
//!
//! The filters have unit tests for the pairs they produce; these prove the
//! pairs survive `reqwest`'s query serializer as repeated keys rather than
//! being collapsed, and that a second page is actually reachable — which is
//! the capability the `Vec<T>` return types removed.

use std::collections::HashMap;

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

/// One equity, complete enough to decode.
fn equity_item(symbol: &str) -> String {
    format!(
        r#"{{
            "id": 1,
            "symbol": "{symbol}",
            "instrument-type": "Equity",
            "short-description": "{symbol}",
            "is-index": false,
            "listed-market": "XNAS",
            "description": "{symbol} Inc",
            "lendability": "Easy To Borrow",
            "market-time-instrument-collection": "Equity",
            "is-closing-only": false,
            "is-options-closing-only": false,
            "active": true,
            "is-illiquid": false,
            "is-etf": false,
            "bypass-manual-review": false,
            "is-fraud-risk": false,
            "streamer-symbol": "{symbol}"
        }}"#
    )
}

/// A paginated envelope: `items` under `data`, `pagination` beside it.
fn paginated_body(items: &[String], page_offset: usize, total_pages: usize) -> String {
    format!(
        r#"{{
            "data": {{ "items": [{}] }},
            "pagination": {{
                "per-page": 2,
                "page-offset": {page_offset},
                "item-offset": {},
                "total-items": {},
                "total-pages": {total_pages},
                "current-item-count": {},
                "previous-link": null,
                "next-link": null,
                "paging-link-template": null
            }},
            "context": "/instruments/equities"
        }}"#,
        items.join(","),
        page_offset * 2,
        total_pages * 2,
        items.len(),
    )
}

/// A venue that answers the token exchange and one listing route.
async fn venue_serving(path: &str, body: String) -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(format!("GET {path}"), Route::ok(body));
    MockVenue::start(routes).await
}

/// The query string of the last non-token request.
fn last_query(venue: &MockVenue) -> String {
    let target = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target != "/oauth/token")
        .expect("the client must have sent a request")
        .target;
    target
        .split_once('?')
        .map(|(_, query)| query.to_string())
        .unwrap_or_default()
}

/// The regression the filter exists for. `product-code` was singular, so a
/// caller could name one product and only one; the venue documents an array.
#[tokio::test]
async fn several_product_codes_reach_the_venue_as_repeated_keys() {
    let venue = venue_serving("/instruments/futures", paginated_body(&[], 0, 1)).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .list_futures(&FutureFilter::for_product_codes(&["ES", "6A"]))
        .await;

    // Brackets are percent-encoded by the query serializer, which is correct
    // and is what the existing `symbol[]` parameters have always sent.
    assert_eq!(
        last_query(&venue),
        "product-code%5B%5D=ES&product-code%5B%5D=6A",
        "each product code must be its own parameter"
    );
}

/// Symbols were already repeated; this pins that the filter did not change it.
#[tokio::test]
async fn equity_symbols_reach_the_venue_as_repeated_keys() {
    let venue = venue_serving("/instruments/equities", paginated_body(&[], 0, 1)).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .list_equities(&EquityFilter::for_symbols(&["AAPL", "MSFT"]))
        .await;

    assert_eq!(last_query(&venue), "symbol%5B%5D=AAPL&symbol%5B%5D=MSFT");
}

/// Every documented equity filter, in one request, in the order the builder
/// writes them.
#[tokio::test]
async fn the_documented_equity_filters_all_reach_the_venue() {
    let venue = venue_serving("/instruments/equities", paginated_body(&[], 3, 9)).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .list_equities(
            &EquityFilter::new()
                .with_page(PageRequest::new().with_page_offset(3).with_per_page(2))
                .with_is_etf(true)
                .with_is_index(false)
                .with_lendability(Lendability::LocateRequired),
        )
        .await;

    // The spaces in `Locate Required` are the venue's own spelling, and the
    // query serializer encodes them as `+`. Asserted rather than glossed over:
    // a filter that sent `Locate%20Required` or `LocateRequired` would look
    // fine in a log and match nothing.
    assert_eq!(
        last_query(&venue),
        "page-offset=3&per-page=2&is-etf=true&is-index=false&lendability=Locate+Required"
    );
}

/// A filter that sets nothing sends nothing, so the venue's own defaults are
/// what answer. A client that always sent `page-offset=0&per-page=1000` had
/// replaced them.
#[tokio::test]
async fn an_unfiltered_listing_sends_an_empty_query() {
    let venue = venue_serving("/instruments/equities", paginated_body(&[], 0, 1)).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client.list_equities(&EquityFilter::new()).await;

    assert_eq!(last_query(&venue), "");
}

/// The capability the `Vec<T>` return type removed: reading past page one.
#[tokio::test]
async fn a_second_page_is_reachable_and_reports_where_it_sits() {
    let venue = venue_serving(
        "/instruments/equities",
        paginated_body(&[equity_item("MSFT"), equity_item("NVDA")], 1, 3),
    )
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let page = client
        .list_equities(&EquityFilter::new().with_page(PageRequest::first().next_page()))
        .await
        .expect("the page must decode");

    assert_eq!(last_query(&venue), "page-offset=1");
    assert_eq!(page.len(), 2);
    assert_eq!(page.pagination.page_offset, 1);
    assert_eq!(page.pagination.total_pages, 3);
    assert!(page.has_more(), "page 1 of 3 is not the last one");

    let symbols: Vec<String> = page.iter().map(|equity| equity.symbol.0.clone()).collect();
    assert_eq!(symbols, vec!["MSFT", "NVDA"]);
}

/// `has_more` is the off-by-one a paging loop gets wrong. Offsets count from
/// zero, so the last page is `total_pages - 1`.
#[tokio::test]
async fn the_last_page_reports_that_it_is_the_last() {
    let venue = venue_serving(
        "/instruments/equities",
        paginated_body(&[equity_item("AAPL")], 2, 3),
    )
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let page = client
        .list_equities(&EquityFilter::new().with_page(PageRequest::new().with_page_offset(2)))
        .await
        .expect("the page must decode");

    assert!(!page.has_more(), "page 2 of 3 is the last one");
}

/// A venue that answers a paginated route without a pagination block is a
/// mismatch the caller can act on. It must not be a panic, and it must not be
/// silently treated as a single page.
#[tokio::test]
async fn a_listing_with_no_pagination_block_is_an_error_and_not_a_panic() {
    let venue = venue_serving(
        "/instruments/equities",
        format!(
            r#"{{"data": {{"items": [{}]}}, "context": "/instruments/equities"}}"#,
            equity_item("AAPL")
        ),
    )
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let outcome = client.list_equities(&EquityFilter::new()).await;

    let error = outcome.expect_err("a missing pagination block must be reported");
    assert!(
        format!("{error}").contains("pagination"),
        "the error must say what was missing: {error}"
    );
}

/// Products paginate too, and their only parameter is the page.
#[tokio::test]
async fn future_products_take_a_page_and_nothing_else() {
    let venue = venue_serving("/instruments/future-products", paginated_body(&[], 4, 5)).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .list_future_products(&PageRequest::new().with_page_offset(4).with_per_page(2))
        .await;

    assert_eq!(last_query(&venue), "page-offset=4&per-page=2");
}

/// The documented `active` filter on the single-option lookup, which the crate
/// did not expose at all.
#[tokio::test]
async fn the_equity_option_lookup_can_ask_about_activity() {
    let venue = venue_serving(
        "/instruments/equity-options/AAPL%20%20241220C00200000",
        String::new(),
    )
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .get_equity_option("AAPL  241220C00200000", Some(false))
        .await;

    assert_eq!(last_query(&venue), "active=false");

    let _ = client
        .get_equity_option("AAPL  241220C00200000", None)
        .await;

    assert_eq!(
        last_query(&venue),
        "",
        "omitting the filter must omit the parameter"
    );
}
