//! Cross-module REST invariants, driven against a loopback venue.
//!
//! These are the properties unit tests cannot reach: they need the real
//! reqwest client, the real envelope decoding and the real error mapping,
//! wired end to end.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::TastyTradeError;
use tastytrade::utils::config::TastyTradeConfig;
use tracing::Level;

use crate::support::{
    CapturedLogs, MockVenue, Route, capture_logs_at, login_response_body, sentinel,
    unparseable_accounts_body,
};

/// A config pointed at `venue`, with credentials that are never real.
fn config_for(venue: &MockVenue) -> TastyTradeConfig {
    TastyTradeConfig {
        username: "someone@example.com".to_string(),
        password: sentinel::PASSWORD.to_string(),
        use_demo: true,
        log_level: "TRACE".to_string(),
        remember_me: true,
        base_url: venue.base_url().to_string(),
        websocket_url: "ws://127.0.0.1:1".to_string(),
    }
}

fn assert_no_secret_leaked(logs: &CapturedLogs) {
    logs.assert_absent(sentinel::SESSION_TOKEN, "the session token");
    logs.assert_absent(sentinel::REMEMBER_TOKEN, "the remember token");
    logs.assert_absent(sentinel::PASSWORD, "the password");
}

#[tokio::test]
async fn login_succeeds_without_writing_a_credential_anywhere() {
    let venue = MockVenue::with_login(login_response_body()).await;
    let config = config_for(&venue);

    // TRACE, so nothing can hide behind a level filter.
    let (client, logs) =
        capture_logs_at(Level::TRACE, async { TastyTrade::login(&config).await }).await;

    let client = client.expect("the canned login response must be accepted");
    assert_no_secret_leaked(&logs);

    // Debug output is part of the contract too: it lands in panic messages and
    // in a consumer's error reports.
    let rendered = format!("{client:?} {client}");
    assert!(
        !rendered.contains(sentinel::SESSION_TOKEN) && !rendered.contains(sentinel::PASSWORD),
        "Debug/Display exposed a credential: {rendered}"
    );

    let sent = venue.requests();
    assert_eq!(sent.len(), 1, "login must be exactly one request");
    assert_eq!(sent[0].method, "POST");
    assert_eq!(sent[0].target, "/sessions");

    // The password belongs in the login body and nowhere else. Asserting it is
    // there is what makes the log assertions above mean something: the value
    // was in play, and still did not get written down.
    assert!(
        sent[0].body.contains(sentinel::PASSWORD),
        "the login request must carry the password it was given"
    );
    assert!(
        sent[0].body.contains("remember-me"),
        "remember_me must be sent kebab-cased: {}",
        sent[0].body
    );
}

#[tokio::test]
async fn missing_credentials_never_reach_the_venue() {
    let venue = MockVenue::with_login(login_response_body()).await;
    let mut config = config_for(&venue);
    config.username = String::new();
    config.password = String::new();

    let error = TastyTrade::login(&config)
        .await
        .expect_err("an empty credential pair must not be posted");

    assert!(
        matches!(error, TastyTradeError::ConfigError(_)),
        "expected a configuration error, got {error:?}"
    );
    assert!(
        venue.requests().is_empty(),
        "the venue was contacted despite missing credentials"
    );
}

#[tokio::test]
async fn an_unparseable_item_is_skipped_without_logging_the_payload() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(unparseable_accounts_body()),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (accounts, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::login(&config)
            .await
            .expect("login must succeed");
        // Account borrows the client, so hand back a count rather than the
        // values themselves.
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    let count = accounts.expect("a skipped item is not a transport failure");
    assert_eq!(count, 0, "the only item cannot deserialize");

    // The whole point: the failure is reported without the data that failed.
    logs.assert_present("failed to deserialize item 0", "the failing item");
    logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    logs.assert_absent(sentinel::NICKNAME, "the account nickname");
    logs.assert_absent(sentinel::BALANCE, "the cash balance");
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn an_account_scoped_error_does_not_carry_the_account_number() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        format!("GET /accounts/{}/balances", sentinel::ACCOUNT_NUMBER),
        Route::status(500, r#"{"error":{"code":"boom","message":"upstream"}}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::login(&config)
        .await
        .expect("login must succeed");

    let error = client
        .get::<serde_json::Value, _>(&format!("/accounts/{}/balances", sentinel::ACCOUNT_NUMBER))
        .await
        .expect_err("a 500 must surface as an error");

    // A well-formed error document is reported as the broker wrote it, so the
    // endpoint context is not in this message. What matters is that nothing
    // added the account number on the way through.
    let rendered = format!("{error}");
    assert!(
        !rendered.contains(sentinel::ACCOUNT_NUMBER),
        "the account number reached the error text: {rendered}"
    );
    assert!(
        rendered.contains("upstream"),
        "the broker message must survive: {rendered}"
    );
}

#[tokio::test]
async fn a_malformed_envelope_is_an_error_not_a_panic() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok("{ this is not json"),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::login(&config)
            .await
            .expect("login must succeed");
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    let error = result.expect_err("a malformed body must not deserialize");
    let rendered = format!("{error}");
    assert!(
        !rendered.contains("this is not json"),
        "the raw body reached the error text: {rendered}"
    );
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn the_session_token_is_sent_but_never_logged() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(r#"{"data":{"items":[]},"context":"/customers/me/accounts"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (accounts, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::login(&config)
            .await
            .expect("login must succeed");
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    assert_eq!(accounts.expect("an empty list is a success"), 0);

    // The token has to travel: it is what authenticates the request. Asserting
    // the request count alone would stay green with the header removed, which
    // would make this test's name a lie.
    let sent = venue.requests();
    assert_eq!(sent.len(), 2);
    let authorization = sent[1]
        .headers
        .get("authorization")
        .expect("the account request must be authenticated");
    assert_eq!(
        authorization,
        sentinel::SESSION_TOKEN,
        "the session token must be the credential actually sent"
    );

    // Sent, and still never written down.
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn a_broker_error_document_becomes_a_typed_error() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        format!("GET /accounts/{}/orders/live", sentinel::ACCOUNT_NUMBER),
        Route::status(
            422,
            format!(
                r#"{{"error":{{"code":"invalid_order","message":"buying power exceeded","errors":[{{"code":"bp","message":"needs {}"}}]}}}}"#,
                sentinel::BALANCE
            ),
        ),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::login(&config)
            .await
            .expect("login must succeed");
        client
            .get::<serde_json::Value, _>(&format!(
                "/accounts/{}/orders/live",
                sentinel::ACCOUNT_NUMBER
            ))
            .await
            .map(|_| ())
    })
    .await;

    let error = result.expect_err("a 422 must surface as an error");

    // The broker's own code and message are what a caller can act on.
    assert!(
        matches!(error, TastyTradeError::Api(_)),
        "expected a typed API error, got {error:?}"
    );
    let displayed = format!("{error}");
    let debugged = format!("{error:?}");
    assert!(
        displayed.contains("buying power exceeded"),
        "the broker summary must survive: {displayed}"
    );
    assert!(
        displayed.contains("bp"),
        "the failing rule's code must survive: {displayed}"
    );

    // The nested detail is where balances and account references live, and
    // ApiError renders as JSON in both Display and Debug, so both are checked.
    assert!(
        !displayed.contains(sentinel::BALANCE),
        "the nested balance is reachable through Display: {displayed}"
    );
    assert!(
        !debugged.contains(sentinel::BALANCE),
        "the nested balance is reachable through Debug: {debugged}"
    );

    // And it must not have been logged on the way through either.
    logs.assert_absent(sentinel::BALANCE, "the balance from the error document");
    logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
}

#[tokio::test]
async fn a_non_json_error_body_degrades_to_status_and_endpoint() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        format!("GET /accounts/{}/balances", sentinel::ACCOUNT_NUMBER),
        Route::status(
            503,
            format!("<html>maintenance {}</html>", sentinel::BALANCE),
        ),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::login(&config)
        .await
        .expect("login must succeed");

    let error = client
        .get::<serde_json::Value, _>(&format!("/accounts/{}/balances", sentinel::ACCOUNT_NUMBER))
        .await
        .expect_err("a 503 must surface as an error");

    let rendered = format!("{error}");
    assert!(
        rendered.contains("503") && rendered.contains("{account}"),
        "the status and the redacted endpoint must survive: {rendered}"
    );
    assert!(
        !rendered.contains(sentinel::BALANCE) && !rendered.contains(sentinel::ACCOUNT_NUMBER),
        "the body reached the error text: {rendered}"
    );
    assert!(
        !format!("{error:?}").contains(sentinel::BALANCE),
        "the body reached Debug"
    );
}
