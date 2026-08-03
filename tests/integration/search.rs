//! Search, end to end against the loopback venue.
//!
//! Three properties the unit tests cannot reach: the query term survives into
//! the **path** encoded (a search for `BRK/B` must not become a search of
//! `/symbols/search/BRK` for `B`), the classification filters arrive
//! comma-joined rather than repeated, and the AI search token never reaches a
//! log line on the way through.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tracing::Level;

use crate::support::{MockVenue, Route, capture_logs_at, sentinel, token_response_body};

/// The Telescope token this venue mints. Distinctive enough that a substring
/// search for it cannot produce a false positive.
const TELESCOPE_TOKEN: &str = "SENTINEL-telescope-token-3Qv7";

fn config_for(venue: &MockVenue) -> TastyTradeConfig {
    TastyTradeConfig {
        client_secret: sentinel::CLIENT_SECRET.into(),
        refresh_token: sentinel::REFRESH_TOKEN.into(),
        client_id: "client-abc".to_string(),
        redirect_uri: "https://app.example.com/cb".to_string(),
        use_demo: true,
        log_level: "TRACE".to_string(),
        base_url: venue.base_url().to_string(),
        websocket_url: "ws://127.0.0.1:1".to_string(),
    }
}

async fn venue_serving(routes: [(&str, Route); 1]) -> MockVenue {
    let mut all = HashMap::new();
    all.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    for (key, route) in routes {
        all.insert(key.to_string(), route);
    }
    MockVenue::start(all).await
}

fn last_target(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target != "/oauth/token")
        .expect("the client must have sent a request")
        .target
}

fn items_body(items: &str) -> String {
    format!(r#"{{"data": {{"items": [{items}]}}, "context": "/search"}}"#)
}

/// A search term is a path segment, and a class separator in one used to
/// select another route entirely.
#[tokio::test]
async fn a_symbol_search_term_is_encoded_into_the_path() {
    let venue = venue_serving([(
        "GET /symbols/search/BRK%2FB",
        Route::ok(items_body(
            r#"{"symbol":"BRK/B","description":"Berkshire Hathaway B","options":true}"#,
        )),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let results = client
        .search_symbols("BRK/B")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(last_target(&venue), "/symbols/search/BRK%2FB");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol, "BRK/B");
    assert_eq!(results[0].options, Some(true));
}

/// A futures symbol begins with the separator, so raw interpolation produced
/// an empty segment and a symbol the router never saw.
#[tokio::test]
async fn a_futures_search_term_keeps_its_leading_separator() {
    let venue = venue_serving([("GET /symbols/search/%2FES", Route::ok(items_body("")))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let results = client.search_symbols("/ES").await.expect("an empty match");

    assert_eq!(last_target(&venue), "/symbols/search/%2FES");
    assert!(results.is_empty(), "a search matching nothing is Ok");
}

/// The encoding trap in the other direction: these filters are **comma-joined
/// into one parameter each**, unlike the instrument listings' repeated keys.
/// Sending them repeated returns results for one value.
#[tokio::test]
async fn classification_filters_arrive_comma_joined_and_not_repeated() {
    let venue = venue_serving([("GET /instruments/search", Route::ok(items_body("")))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let _ = client
        .search_instruments(
            &InstrumentSearchFilter::for_query("gold")
                .with_types(&["Equity", "Future"])
                .with_instrument_sub_types(&["ETF", "Index"])
                .with_limit(10),
        )
        .await;

    let query = last_target(&venue)
        .split_once('?')
        .map(|(_, query)| query.to_string())
        .unwrap_or_default();

    assert_eq!(
        query,
        "query=gold&type=Equity%2CFuture&instrument-sub-type=ETF%2CIndex&limit=10"
    );
    assert!(
        !query.contains("type%5B%5D"),
        "these filters are not array parameters: {query}"
    );
}

/// A local refusal sends nothing. Asserted by the absence of a request, not
/// only by the error type — an error that still reached the venue would be a
/// different bug with the same message.
#[tokio::test]
async fn an_over_large_limit_never_reaches_the_venue() {
    let venue = venue_serving([("GET /instruments/search", Route::ok(items_body("")))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = client
        .search_instruments(&InstrumentSearchFilter::new().with_limit(MAX_SEARCH_RESULTS + 1))
        .await
        .expect_err("the cap must be enforced locally");

    assert!(!error.is_retryable());
    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.target == "/oauth/token"),
        "nothing beyond authentication may have been sent: {:?}",
        venue.requests()
    );
}

/// The token is a credential. It travels from the socket through the decoder
/// into a value the caller holds, and must appear at no point in between.
#[tokio::test]
async fn the_ai_search_token_reaches_no_log_line_or_rendering() {
    let venue = venue_serving([(
        "POST /instruments/ai-search-token",
        Route::ok(format!(
            r#"{{"data": {{"token":"{TELESCOPE_TOKEN}",
                 "expires-at":"2026-08-03T12:00:00.000+00:00"}},
                "context": "/instruments/ai-search-token"}}"#
        )),
    )])
    .await;
    let config = config_for(&venue);

    // TRACE, so nothing hides behind a level filter.
    let (token, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config).await?;
        client.ai_search_token().await
    })
    .await;

    let token = token.expect("the venue answered with a token");

    logs.assert_absent(TELESCOPE_TOKEN, "the AI search token");

    let rendered = format!("{token:?} {token}");
    assert!(
        !rendered.contains(TELESCOPE_TOKEN),
        "Debug or Display exposed the token: {rendered}"
    );

    // …and it is still usable, which is the point of redacting rather than
    // discarding.
    assert_eq!(
        token.field("token").and_then(serde_json::Value::as_str),
        Some(TELESCOPE_TOKEN)
    );
    assert!(token.expires_at().is_some());
}

/// The endpoint takes no body, and `()` would have serialised to `null` —
/// a different request from an empty object.
#[tokio::test]
async fn minting_a_token_posts_an_empty_object() {
    let venue = venue_serving([(
        "POST /instruments/ai-search-token",
        Route::ok(r#"{"data": {}, "context": "/instruments/ai-search-token"}"#),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let token = client
        .ai_search_token()
        .await
        .expect("an empty object is still a response");

    let request = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target != "/oauth/token")
        .expect("a request must have been sent");

    assert_eq!(request.method, "POST");
    assert_eq!(request.body, "{}");
    assert!(token.is_empty(), "an empty answer is recognisable as one");
}
