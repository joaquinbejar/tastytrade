//! What actually goes on the wire when a symbol is not path-safe.
//!
//! The encoder has unit tests in `src/api/url.rs`; these prove the encoded
//! value survives everything between the call site and the socket. That gap is
//! where the bug would live now: `reqwest` parses the URL it is handed and
//! percent-encodes what it considers unsafe, so a call site that encoded
//! nothing produced a *valid* request against the wrong route rather than a
//! failure anybody would notice.
//!
//! Most of these let the venue answer 404. The assertion is on what was sent,
//! and inventing a response body per endpoint would add nothing to it.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::utils::config::TastyTradeConfig;
use tracing::Level;

use crate::support::{
    CapturedLogs, MockVenue, Route, capture_logs_at, sentinel, token_response_body,
};

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

/// A venue that mints a token and 404s everything else, plus a client on it.
async fn client_on_a_venue() -> (MockVenue, TastyTrade) {
    let venue = MockVenue::with_token(token_response_body()).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("the canned token response must be accepted");
    (venue, client)
}

/// The request target of the last non-token request.
fn last_target(venue: &MockVenue) -> String {
    venue
        .requests()
        .into_iter()
        .rfind(|request| request.target != "/oauth/token")
        .expect("the client must have sent a request")
        .target
}

/// A `GET /customers/me/accounts` body carrying an account number that is not
/// path-safe.
///
/// Contrived on purpose: the venue issues plain alphanumeric numbers today, so
/// this is the case that has never happened rather than the case that has. It
/// is here because the failure would be silent — a separator inside the number
/// splits the path *and* defeats the redaction that reads it.
fn account_body_with_separator(number: &str) -> String {
    format!(
        r#"{{
            "data": {{
                "items": [
                    {{
                        "account": {{
                            "account-number": "{number}",
                            "nickname": "Test",
                            "account-type-name": "Individual",
                            "margin-or-cash": "Margin",
                            "opened-at": "2025-01-14T10:22:41.000+00:00"
                        }},
                        "authority-level": "owner"
                    }}
                ]
            }},
            "context": "/customers/me/accounts"
        }}"#
    )
}

/// A futures symbol begins with the separator, so this is the endpoint where
/// raw interpolation produced `/instruments/futures//ESZ4` — an empty segment
/// followed by a symbol the router never saw.
#[tokio::test]
async fn a_futures_symbol_keeps_its_leading_separator_encoded() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_future("/ESZ4").await;

    assert_eq!(last_target(&venue), "/instruments/futures/%2FESZ4");
}

/// The crypto pair separator. This site already encoded `/` by hand; the point
/// of the test is that it still does after the hand-rolled version was removed.
#[tokio::test]
async fn a_crypto_pair_separator_is_encoded() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_cryptocurrency("BTC/USD").await;

    assert_eq!(
        last_target(&venue),
        "/instruments/cryptocurrencies/BTC%2FUSD"
    );
}

/// A future option carries a leading `./` and two spaces. `.` is unreserved,
/// so it stays literal — RFC 3986 §2.3 says a producer should not encode it,
/// and the segment is never *exactly* `.` or `..`, which is the only form a
/// URL parser treats specially.
#[tokio::test]
async fn a_future_option_symbol_survives_its_dots_and_spaces() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_future_option("./ESZ4 EW4U4 240927P5520").await;

    assert_eq!(
        last_target(&venue),
        "/instruments/future-options/.%2FESZ4%20EW4U4%20240927P5520"
    );
}

/// Equities encoded nothing at all before this. `BRK/B` selected
/// `/instruments/equities/BRK` with a trailing `/B`, which is a different
/// route rather than an error.
#[tokio::test]
async fn an_equity_class_separator_no_longer_selects_another_route() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_equity("BRK/B").await;

    assert_eq!(last_target(&venue), "/instruments/equities/BRK%2FB");
}

/// Option chains take the underlying as a path segment in three shapes.
#[tokio::test]
async fn every_option_chain_shape_encodes_its_underlying() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.option_chain_for("BRK/B").await;
    assert_eq!(last_target(&venue), "/option-chains/BRK%2FB");

    let _ = client.nested_option_chain_for("BRK/B").await;
    assert_eq!(last_target(&venue), "/option-chains/BRK%2FB/nested");

    let _ = client.get_compact_option_chain("BRK/B").await;
    assert_eq!(last_target(&venue), "/option-chains/BRK%2FB/compact");
}

/// The characters that end a path rather than extend it. A surviving `?` turns
/// the rest of the symbol into a query string and the venue answers about a
/// different instrument; a `#` never reaches the venue at all.
#[tokio::test]
async fn a_query_or_fragment_delimiter_cannot_escape_the_path() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_equity("A?b").await;
    assert_eq!(last_target(&venue), "/instruments/equities/A%3Fb");

    let _ = client.get_equity("A#b").await;
    assert_eq!(last_target(&venue), "/instruments/equities/A%23b");
}

/// Non-ASCII goes out as UTF-8 octets, not as whatever the HTTP client would
/// have improvised.
#[tokio::test]
async fn a_non_ascii_symbol_is_encoded_as_utf8_octets() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_equity("AÑ").await;

    assert_eq!(last_target(&venue), "/instruments/equities/A%C3%91");
}

/// The other direction: a symbol that already looks encoded is data, not
/// markup. `%2F` is a three-character symbol here, and encoding it once more
/// is correct — what must not happen is a *second* pass over a value this
/// crate encoded itself.
#[tokio::test]
async fn an_input_that_looks_encoded_is_treated_as_literal_text() {
    let (venue, client) = client_on_a_venue().await;

    let _ = client.get_equity("BTC%2FUSD").await;

    let target = last_target(&venue);
    assert_eq!(target, "/instruments/equities/BTC%252FUSD");

    // And the plain form goes out singly encoded, which is the assertion that
    // would fail if any call site kept its old `.replace()` underneath the
    // shared encoder.
    let _ = client.get_equity("BTC/USD").await;
    let target = last_target(&venue);
    assert_eq!(target, "/instruments/equities/BTC%2FUSD");
    assert!(
        !target.contains("%252F"),
        "the symbol was encoded twice: {target}"
    );
}

/// The path the client sends is the path the server sees. Everything else here
/// asserts on what was recorded; this one registers the encoded route and
/// requires it to match, which is the part that would break if `reqwest` were
/// normalising the escape on the way out.
#[tokio::test]
async fn an_encoded_path_reaches_the_route_it_names() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /instruments/equity-options/BRK%2FB%20240927C500".to_string(),
        Route::ok(
            r#"{"data":{"streamer-symbol":".BRK/B240927C500"},
                "context":"/instruments/equity-options"}"#,
        ),
    );
    let venue = MockVenue::start(routes).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("the canned token response must be accepted");

    let info = client
        .get_option_info("BRK/B 240927C500")
        .await
        .expect("the encoded path must select the route it names");

    assert_eq!(info.streamer_symbol.0, ".BRK/B240927C500");
}

/// Account-scoped paths, where encoding and redaction are the same problem.
///
/// A separator inside the account number used to split the path into two
/// segments. `redact_account_path` replaces the segment *after* `accounts`, so
/// it replaced the first half and let the second half through — the identifier
/// partially survived into an error that travels wherever the caller sends it.
#[tokio::test]
async fn an_account_number_stays_one_segment_and_stays_redacted() {
    let separator_number = "5WX/00042";
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(account_body_with_separator(separator_number)),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (outcome, logs): (Result<_, _>, CapturedLogs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config).await?;
        let accounts = client.accounts().await?;
        let account = accounts.into_iter().next().expect("one account");
        account.balance().await
    })
    .await;

    // The venue has no balances route, so this is the failure path — which is
    // the one that produces an error a caller can print.
    let error = outcome.expect_err("the loopback venue answers 404 for balances");

    assert_eq!(
        last_target(&venue),
        "/accounts/5WX%2F00042/balances",
        "the number must be one encoded segment"
    );

    let rendered = format!("{error} {error:?}");
    assert!(
        rendered.contains("{account}"),
        "the account segment must be redacted: {rendered}"
    );
    assert!(
        !rendered.contains("00042"),
        "part of the account number survived redaction: {rendered}"
    );
    logs.assert_absent("00042", "the account number");
}
