//! Cross-module REST invariants, driven against a loopback venue.
//!
//! These are the properties unit tests cannot reach: they need the real
//! reqwest client, the real envelope decoding and the real error mapping,
//! wired end to end.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::prelude::ActiveEquityFilter;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::{Environment, TastyTradeError};
use tracing::Level;

use crate::support::{
    CapturedLogs, MockVenue, Route, capture_logs_at, expiring_token_response_body,
    partially_unparseable_accounts_body, sentinel, token_error_body, token_response_body,
    wholly_unparseable_accounts_body,
};

/// A config pointed at `venue`, with credentials that are never real.
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

/// Routes that answer the token exchange plus whatever else a test needs.
fn routes_with_token<const N: usize>(extra: [(&str, Route); N]) -> HashMap<String, Route> {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    for (key, route) in extra {
        routes.insert(key.to_string(), route);
    }
    routes
}

fn assert_no_secret_leaked(logs: &CapturedLogs) {
    logs.assert_absent(sentinel::ACCESS_TOKEN, "the access token");
    logs.assert_absent(sentinel::REFRESH_TOKEN, "the refresh token");
    logs.assert_absent(sentinel::CLIENT_SECRET, "the client secret");
    logs.assert_absent(sentinel::ID_TOKEN, "the identity token");
}

#[tokio::test]
async fn authenticating_writes_no_credential_anywhere() {
    let venue = MockVenue::with_token(token_response_body()).await;
    let config = config_for(&venue);

    // TRACE, so nothing can hide behind a level filter.
    let (client, logs) =
        capture_logs_at(Level::TRACE, async { TastyTrade::connect(&config).await }).await;

    let client = client.expect("the canned token response must be accepted");
    assert_no_secret_leaked(&logs);

    // Debug output is part of the contract too: it lands in panic messages and
    // in a consumer's error reports.
    let rendered = format!("{client:?} {client}");
    assert!(
        !rendered.contains("SENTINEL"),
        "Debug/Display exposed a credential: {rendered}"
    );

    let sent = venue.requests();
    assert_eq!(sent.len(), 1, "authenticating must be exactly one request");
    assert_eq!(sent[0].method, "POST");
    assert_eq!(sent[0].target, "/oauth/token");

    // RFC 6749 §6: the token endpoint takes form-encoded parameters, and the
    // client must not let a JSON default shadow that.
    assert_eq!(
        sent[0].headers.get("content-type").map(String::as_str),
        Some("application/x-www-form-urlencoded"),
        "the token request must be form-encoded: {:?}",
        sent[0].headers
    );

    // The venue rejects a User-Agent that is not <product>/<version>, and
    // rejects a missing one outright.
    let agent = sent[0]
        .headers
        .get("user-agent")
        .expect("every request carries a user agent");
    assert!(
        agent.split_once('/').is_some_and(|(product, version)| {
            !product.is_empty() && version.chars().next().is_some_and(|c| c.is_ascii_digit())
        }),
        "the user agent must be <product>/<version>: {agent}"
    );

    // The secrets belong in the token request body and nowhere else. Asserting
    // they are there is what makes the log assertions above mean something:
    // the values were in play, and still did not get written down.
    assert!(
        sent[0].body.contains("grant_type=refresh_token"),
        "the grant type must be the refresh flow: {}",
        sent[0].body
    );
    assert!(
        sent[0].body.contains(sentinel::CLIENT_SECRET)
            && sent[0].body.contains(sentinel::REFRESH_TOKEN),
        "the token request must carry the credentials it was given"
    );
    // The authorization-code parameters belong to the other grant.
    assert!(
        !sent[0].body.contains("redirect_uri"),
        "the refresh grant sends no redirect URI: {}",
        sent[0].body
    );
}

/// Every request after the first has to present the token as a bearer
/// credential. Without the prefix the venue answers 401 on everything.
#[tokio::test]
async fn every_request_carries_the_bearer_token() {
    let venue = MockVenue::start(routes_with_token([(
        "GET /customers/me/accounts",
        Route::ok(r#"{"data":{"items":[]},"context":"/customers/me/accounts"}"#),
    )]))
    .await;
    let config = config_for(&venue);

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");
    client.accounts().await.expect("an empty listing is fine");

    let sent = venue.requests();
    let listing = sent
        .iter()
        .find(|request| request.target == "/customers/me/accounts")
        .expect("the listing was requested");

    assert_eq!(
        listing.headers.get("authorization").map(String::as_str),
        Some(format!("Bearer {}", sentinel::ACCESS_TOKEN).as_str()),
        "the access token must be presented as a bearer credential"
    );
}

/// The token lasts fifteen minutes and the client is meant to outlive it, so
/// a stale token is replaced before the request that would have failed on it.
#[tokio::test]
async fn an_expiring_token_is_refreshed_before_the_next_request() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(expiring_token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(r#"{"data":{"items":[]},"context":"/customers/me/accounts"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");
    client.accounts().await.expect("an empty listing is fine");

    let exchanges = venue
        .requests()
        .iter()
        .filter(|request| request.target == "/oauth/token")
        .count();
    assert_eq!(
        exchanges, 2,
        "a token already inside the refresh margin must be renewed before it is used"
    );
}

/// A refused refresh is terminal: the same secret produces the same answer,
/// and both streamers stop rather than back off on `Auth`.
#[tokio::test]
async fn a_refused_grant_is_an_authentication_failure_and_is_not_retryable() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::status(400, token_error_body("invalid_grant")),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) =
        capture_logs_at(Level::TRACE, async { TastyTrade::connect(&config).await }).await;
    let error = result.expect_err("a refused grant is not a session");

    assert!(
        matches!(error, TastyTradeError::Auth(_)),
        "expected an authentication failure, got {error:?}"
    );
    assert!(!error.is_retryable(), "presenting it again cannot help");

    let rendered = format!("{error}");
    assert!(rendered.contains("invalid_grant"), "{rendered}");
    // The refusal document echoed a credential back in `error_description`,
    // and the sentinel body also puts one in `error` itself. Neither field is
    // trusted: only the spec's own codes leave the type.
    assert!(!rendered.contains(sentinel::REFRESH_TOKEN), "{rendered}");
    // Which deployment refused is part of what makes the message actionable.
    // A loopback host is reported as production, the answer that fails safe.
    assert!(rendered.contains("production"), "{rendered}");
    // The venue's error_description echoed a credential back. It must not
    // travel any further than the socket it arrived on.
    assert!(!rendered.contains("SENTINEL"), "{rendered}");
    assert_no_secret_leaked(&logs);
}

/// `error` is untrusted response text. An endpoint that echoes a credential
/// back in that field must not get it into the error a caller logs, and the
/// classification must not turn an unreadable reply into a dead session.
#[tokio::test]
async fn an_error_code_the_spec_does_not_define_never_reaches_the_caller() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::status(400, format!(r#"{{"error":"{}"}}"#, sentinel::CLIENT_SECRET)),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) =
        capture_logs_at(Level::TRACE, async { TastyTrade::connect(&config).await }).await;
    let error = result.expect_err("a 400 is not a session");

    let rendered = format!("{error} {error:?}");
    assert!(
        !rendered.contains("SENTINEL"),
        "the code reached the caller: {rendered}"
    );
    assert_no_secret_leaked(&logs);

    // Not classified as a credential failure: this crate could not read the
    // code, so giving up on the grant would be a guess.
    assert!(
        !matches!(error, TastyTradeError::Auth(_)),
        "an unreadable code must not be treated as a dead credential: {error:?}"
    );
}

/// A token endpoint that is down says nothing about whether the secret is
/// good, so this keeps its request shape and stays retryable.
#[tokio::test]
async fn a_token_endpoint_outage_is_not_a_credential_failure() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::status(503, r#"{"error":"temporarily_unavailable"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let error = TastyTrade::connect(&config)
        .await
        .expect_err("a 503 is not a session");

    match &error {
        TastyTradeError::Request { context, .. } => {
            assert_eq!(context.status, Some(503));
            // A loopback host is not the certification one, so the environment
            // derivation reports production. That is the answer that fails
            // safe, and it is the same one every other verb reports here.
            assert_eq!(context.environment, Environment::Production);
            assert!(context.operation.contains("/oauth/token"), "{context}");
        }
        other => panic!("expected a request failure, got {other:?}"),
    }
    assert!(error.is_retryable(), "a 503 is worth trying again");
}

#[tokio::test]
async fn missing_credentials_never_reach_the_venue() {
    let venue = MockVenue::with_token(token_response_body()).await;
    let mut config = config_for(&venue);
    config.client_secret = "".into();
    config.refresh_token = "".into();

    let error = TastyTrade::connect(&config)
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
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(partially_unparseable_accounts_body()),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (accounts, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
        // Account borrows the client, so hand back a count rather than the
        // values themselves.
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    let count = accounts.expect("a skipped item is not a transport failure");
    assert_eq!(count, 1, "the healthy account survives the broken one");

    // The whole point: the failure is reported without the data that failed.
    logs.assert_present("failed to deserialize item 1", "the failing item");
    logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    logs.assert_absent(sentinel::NICKNAME, "the account nickname");
    logs.assert_absent(sentinel::BALANCE, "the cash balance");
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn an_account_scoped_error_does_not_carry_the_account_number() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        format!("GET /accounts/{}/balances", sentinel::ACCOUNT_NUMBER),
        Route::status(500, r#"{"error":{"code":"boom","message":"upstream"}}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");

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
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok("{ this is not json"),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
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
async fn the_access_token_is_sent_but_never_logged() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(r#"{"data":{"items":[]},"context":"/customers/me/accounts"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (accounts, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
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
        &format!("Bearer {}", sentinel::ACCESS_TOKEN),
        "the access token must be the credential actually sent"
    );

    // Sent, and still never written down.
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn a_broker_error_document_becomes_a_typed_error() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
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
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
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

    // The broker's own code and message are what a caller can act on, and the
    // context says where and against which deployment.
    let TastyTradeError::Request { context, api } = &error else {
        panic!("expected a request error, got {error:?}");
    };
    assert_eq!(context.status, Some(422));
    assert_eq!(context.method, "GET");
    // The fixture sets use_demo: true beside a loopback base_url, which is the
    // divergence the environment derivation exists to survive. The URL decides,
    // and a host that is not the certification one is reported as production
    // because that is the answer that fails safe.
    assert_eq!(
        context.environment,
        Environment::Production,
        "the reported environment must follow the URL the request actually used"
    );
    assert!(
        context.operation.contains("{account}"),
        "the operation must be redacted: {}",
        context.operation
    );
    assert_eq!(
        api.as_ref().map(|a| a.message.as_str()),
        Some("buying power exceeded")
    );
    assert!(
        !error.is_retryable(),
        "a 422 is the venue rejecting the request, not asking to be retried"
    );

    // The nested detail is where balances and account references live, and
    // ApiError renders as JSON in both Display and Debug, so both are checked.
    let displayed = format!("{error}");
    let debugged = format!("{error:?}");
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
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
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

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");

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
        error.is_retryable(),
        "a 503 is the venue asking to be tried again: {rendered}"
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

#[tokio::test]
async fn a_successful_body_is_never_written_to_the_logs() {
    let account_body = format!(
        r#"{{"data":{{"items":[{{"account":{{"account-number":"{}","nickname":"Main","account-type-name":"Individual","margin-or-cash":"Margin","opened-at":"2025-01-14T10:22:41.000+00:00","is-closed":false}},"authority-level":"owner"}}]}},"context":"/customers/me/accounts"}}"#,
        sentinel::ACCOUNT_NUMBER
    );

    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(account_body),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    // TRACE: if a body survives anywhere, it survives here.
    let (count, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    assert_eq!(
        count.expect("the account must deserialize"),
        1,
        "the fixture is only meaningful if the body actually parsed"
    );

    // The exchange is still observable, just not its contents.
    logs.assert_present("bytes in", "the response metadata");
    logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    logs.assert_absent("nickname", "a field name from the response body");
    assert_no_secret_leaked(&logs);
}

#[tokio::test]
async fn a_listing_without_pagination_is_an_error_not_a_panic() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    // A well-formed envelope that simply carries no pagination block, which
    // used to reach an expect() and abort the caller's process.
    routes.insert(
        "GET /instruments/equities/active".to_string(),
        Route::ok(r#"{"data":{"items":[]},"context":"/instruments/equities/active"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");

    let error = client
        .list_active_equities(&ActiveEquityFilter::new())
        .await
        .expect_err("a missing pagination block must not panic");

    let rendered = format!("{error}");
    assert!(
        rendered.contains("pagination"),
        "the error must say what was missing: {rendered}"
    );
}

/// `context` comes from the venue and mirrors the request path, so on an
/// account-scoped endpoint it carries the account number straight into the
/// error text unless it goes through the same redaction as everything else.
#[tokio::test]
async fn the_pagination_error_redacts_the_venue_supplied_context() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        format!(
            "GET /accounts/{}/balance-snapshots",
            sentinel::ACCOUNT_NUMBER
        ),
        Route::ok(format!(
            r#"{{"data":{{"items":[]}},"context":"/accounts/{}/balance-snapshots"}}"#,
            sentinel::ACCOUNT_NUMBER
        )),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::connect(&config)
        .await
        .expect("authentication must succeed");

    let error = client
        .get_with_query::<tastytrade::api::base::Items<serde_json::Value>, tastytrade::api::base::Paginated<serde_json::Value>, _>(
            &format!("/accounts/{}/balance-snapshots", sentinel::ACCOUNT_NUMBER),
            &[],
        )
        .await
        .expect_err("a missing pagination block must not panic");

    let rendered = format!("{error}");
    assert!(
        !rendered.contains(sentinel::ACCOUNT_NUMBER),
        "the venue-supplied context carried the account number: {rendered}"
    );
    assert!(
        rendered.contains("{account}"),
        "the redacted context should still identify the endpoint: {rendered}"
    );
}

/// The shape that made a missing field read as an authentication problem: a
/// real account existed, and the caller was handed an empty list.
#[tokio::test]
async fn a_listing_where_nothing_decodes_is_reported_not_returned_empty() {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(wholly_unparseable_accounts_body()),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let (result, logs) = capture_logs_at(Level::WARN, async {
        let client = TastyTrade::connect(&config)
            .await
            .expect("authentication must succeed");
        client.accounts().await.map(|accounts| accounts.len())
    })
    .await;

    let error = result.expect_err("a wholly unparseable listing must not look empty");
    let rendered = format!("{error}");
    assert!(
        rendered.contains("failed to deserialize") && rendered.contains("all 1 item(s)"),
        "the error must say the model is the problem and how much was lost: {rendered}"
    );
    assert!(
        !rendered.contains(sentinel::ACCOUNT_NUMBER),
        "the error must not carry the payload: {rendered}"
    );
    logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
}

/// Placement evidence: the receipt is bound to the account it was reviewed
/// against, and the warnings cannot be skipped past without saying so.
mod reviewed_placement {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use tastytrade::{
        Action, InstrumentType, Order, OrderBuilder, OrderLegBuilder, OrderType, PriceEffect,
        TimeInForce,
    };

    fn an_order() -> Order {
        let leg = OrderLegBuilder::default()
            .instrument_type(InstrumentType::Equity)
            .symbol("AAPL")
            .quantity(Decimal::from(1))
            .action(Action::BuyToOpen)
            .build()
            .expect("a one-share buy is valid");

        OrderBuilder::default()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(Decimal::from_str("1.25").unwrap())
            .price_effect(PriceEffect::Debit)
            .legs(vec![leg])
            .build()
            .expect("a limit order with a price and a leg is valid")
    }

    fn dry_run_body(warnings: &str) -> String {
        format!(
            r#"{{"data":{{
                "order":{{"account-number":"5WX00001","time-in-force":"Day","order-type":"Limit",
                          "size":1,"underlying-symbol":"AAPL","price":1.25,
                          "price-effect":"Debit","status":"Received","cancellable":true,
                          "editable":true,"edited":false,"legs":[]}},
                "warnings":[{warnings}],
                "buying-power-effect":{{"change-in-margin-requirement":125.0,
                    "change-in-margin-requirement-effect":"Debit",
                    "change-in-buying-power":125.0,"change-in-buying-power-effect":"Debit",
                    "current-buying-power":1000.0,"current-buying-power-effect":"Credit",
                    "new-buying-power":875.0,"new-buying-power-effect":"Credit",
                    "isolated-order-margin-requirement":125.0,
                    "isolated-order-margin-requirement-effect":"Debit",
                    "is-spread":false,"impact":125.0,"effect":"Debit"}},
                "fee-calculation":{{"regulatory-fees":0.0,"regulatory-fees-effect":"None",
                    "clearing-fees":0.0,"clearing-fees-effect":"None","commission":0.0,
                    "commission-effect":"None","proprietary-index-option-fees":0.0,
                    "proprietary-index-option-fees-effect":"None","total-fees":0.0,
                    "total-fees-effect":"None"}}
            }},"context":"/accounts/5WX00001/orders/dry-run"}}"#
        )
    }

    async fn account_venue(warnings: &str) -> MockVenue {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            "GET /customers/me/accounts".to_string(),
            Route::ok(
                r#"{"data":{"items":[
                   {"account":{"account-number":"5WX00001","nickname":"Main",
                    "account-type-name":"Individual","margin-or-cash":"Margin",
                    "opened-at":"2025-01-14T10:22:41.000+00:00"},"authority-level":"owner"},
                   {"account":{"account-number":"5WX00002","nickname":"Second",
                    "account-type-name":"Individual","margin-or-cash":"Margin",
                    "opened-at":"2025-02-01T10:22:41.000+00:00"},"authority-level":"owner"}]},
                   "context":"/customers/me/accounts"}"#,
            ),
        );
        routes.insert(
            "POST /accounts/5WX00001/orders/dry-run".to_string(),
            Route::ok(dry_run_body(warnings)),
        );
        MockVenue::start(routes).await
    }

    #[tokio::test]
    async fn a_clean_dry_run_accepts_without_ceremony() {
        let venue = account_venue("").await;
        let config = config_for(&venue);
        let client = TastyTrade::connect(&config).await.expect("authentication");
        let accounts = client.accounts().await.expect("accounts");
        let account = accounts.first().expect("one account");

        let receipt = account
            .review_order(&an_order())
            .await
            .expect("the dry run succeeds");

        assert!(receipt.warnings().is_empty());
        receipt.accept().expect("a clean dry run accepts");
    }

    #[tokio::test]
    async fn warnings_must_be_acknowledged_by_name() {
        let venue = account_venue(
            r#"{"code":"tif_next_valid_sesssion","message":"Placed at the next session."}"#,
        )
        .await;
        let config = config_for(&venue);
        let client = TastyTrade::connect(&config).await.expect("authentication");
        let accounts = client.accounts().await.expect("accounts");
        let account = accounts.first().expect("one account");

        let receipt = account
            .review_order(&an_order())
            .await
            .expect("the dry run succeeds");

        assert_eq!(receipt.warnings().len(), 1);
        assert_eq!(receipt.warnings()[0].message, "Placed at the next session.");

        assert!(!receipt.is_clean());

        // Saying so out loud is the only way through.
        let reviewed = receipt.accept_with_warnings();
        assert_eq!(reviewed.account_number().0, "5WX00001");
    }

    /// Buying power, permissions and positions are per account, so a review
    /// against one says nothing about another. The guard fires before any
    /// request, which is what the request count asserts.
    #[tokio::test]
    async fn a_receipt_cannot_be_spent_on_another_account() {
        let venue = account_venue("").await;
        let config = config_for(&venue);
        let client = TastyTrade::connect(&config).await.expect("authentication");
        let accounts = client.accounts().await.expect("accounts");
        assert_eq!(accounts.len(), 2, "the fixture needs two accounts");

        let reviewed = accounts[0]
            .review_order(&an_order())
            .await
            .expect("the dry run succeeds")
            .accept()
            .expect("a clean dry run accepts");

        let before = venue.requests().len();
        let error = accounts[1]
            .place_reviewed_order(reviewed)
            .await
            .expect_err("a receipt from another account must not place");

        assert!(
            format!("{error}").contains("different account"),
            "the error should say what is wrong: {error}"
        );
        assert_eq!(
            venue.requests().len(),
            before,
            "the guard must fire before anything reaches the venue"
        );
    }

    /// An account number is text, and certification reuses production
    /// numbering. Without binding the receipt to the venue that answered, a
    /// sandbox dry run would authorise a real order against the same number.
    #[tokio::test]
    async fn a_receipt_cannot_be_spent_on_another_venue() {
        let first = account_venue("").await;
        let second = account_venue("").await;
        assert_ne!(
            first.base_url(),
            second.base_url(),
            "the two venues must be distinguishable"
        );

        let reviewed = {
            let config = config_for(&first);
            let client = TastyTrade::connect(&config).await.expect("authentication");
            let accounts = client.accounts().await.expect("accounts");
            accounts[0]
                .review_order(&an_order())
                .await
                .expect("the dry run succeeds")
                .accept()
                .expect("a clean dry run accepts")
        };

        // Same account number, different venue.
        let config = config_for(&second);
        let client = TastyTrade::connect(&config).await.expect("authentication");
        let accounts = client.accounts().await.expect("accounts");
        assert_eq!(accounts[0].number().0, reviewed.account_number().0);

        let before = second.requests().len();
        let error = accounts[0]
            .place_reviewed_order(reviewed)
            .await
            .expect_err("a receipt from another venue must not place");

        assert!(
            format!("{error}").contains("different venue"),
            "the error should say what is wrong: {error}"
        );
        assert_eq!(
            second.requests().len(),
            before,
            "the guard must fire before anything reaches the venue"
        );
    }
}

/// Every verb reports a failure the same way.
///
/// GET has always produced a typed `Request` error with a status, a redacted
/// endpoint and an environment. POST, DELETE and the authentication request
/// never inspected the status at all: they handed the body straight to serde
/// and surfaced whatever it said about a document it could not decode. These
/// pin the shared path.
mod every_verb_reports_failures_alike {
    use super::*;

    /// The verb that can place an order.
    #[tokio::test]
    async fn a_rejected_post_carries_the_status_and_a_redacted_endpoint() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            format!("POST /accounts/{}/orders", sentinel::ACCOUNT_NUMBER),
            Route::status(
                422,
                format!(
                    r#"{{"error":{{"code":"buying_power","message":"Order exceeds buying power",
                         "errors":[{{"code":"bp","message":"needs {}"}}]}}}}"#,
                    sentinel::BALANCE
                ),
            ),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::connect(&config)
                .await
                .expect("authentication must succeed");
            client
                .post::<serde_json::Value, _, _>(
                    format!("/accounts/{}/orders", sentinel::ACCOUNT_NUMBER),
                    serde_json::json!({ "order-type": "Market" }),
                )
                .await
                .map(|_| ())
        })
        .await;

        let error = result.expect_err("a 422 must surface as an error");
        let TastyTradeError::Request { context, api } = &error else {
            panic!("expected a request error, got {error:?}");
        };
        assert_eq!(context.status, Some(422));
        assert_eq!(context.method, "POST");
        assert!(
            context.operation.contains("{account}")
                && !context.operation.contains(sentinel::ACCOUNT_NUMBER),
            "the account number must not survive into the error: {}",
            context.operation
        );
        assert_eq!(
            api.as_ref().map(|a| a.message.as_str()),
            Some("Order exceeds buying power")
        );

        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::BALANCE),
            "the buying-power figure reached the error: {rendered}"
        );
        logs.assert_absent(sentinel::BALANCE, "the buying-power figure");
        logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    }

    /// The verb that can cancel an order. A 404 here means the order is already
    /// gone, which a caller wants to tell apart from a network failure.
    #[tokio::test]
    async fn a_rejected_delete_carries_the_status_and_a_redacted_endpoint() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::connect(&config)
                .await
                .expect("authentication must succeed");
            // Unrouted, so the venue answers 404 with a JSON error body.
            client
                .delete::<serde_json::Value, _>(format!(
                    "/accounts/{}/orders/17",
                    sentinel::ACCOUNT_NUMBER
                ))
                .await
                .map(|_| ())
        })
        .await;

        let error = result.expect_err("a 404 must surface as an error");
        let TastyTradeError::Request { context, .. } = &error else {
            panic!("expected a request error, got {error:?}");
        };
        assert_eq!(context.status, Some(404));
        assert_eq!(context.method, "DELETE");
        assert!(
            context.operation.contains("{account}")
                && !context.operation.contains(sentinel::ACCOUNT_NUMBER),
            "the account number must not survive into the error: {}",
            context.operation
        );
        assert!(
            !error.is_retryable(),
            "an order that is not there will not be there on a retry"
        );
        logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    }

    /// A venue that answers `200` with an error document disagrees with itself.
    /// The document is the more specific answer, and treating the response as a
    /// success would hand the caller a decode failure for a body that plainly
    /// says what went wrong.
    #[tokio::test]
    async fn a_success_status_carrying_an_error_document_is_an_error() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            "POST /accounts/5WX00001/orders".to_string(),
            Route::ok(r#"{"error":{"code":"preflight","message":"Market closed"}}"#),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::connect(&config).await.expect("authentication");
        let error = client
            .post::<serde_json::Value, _, _>(
                "/accounts/5WX00001/orders",
                serde_json::json!({ "order-type": "Market" }),
            )
            .await
            .expect_err("an error document is an error whatever the status says");

        let TastyTradeError::Request { context, api } = &error else {
            panic!("expected a request error, got {error:?}");
        };
        assert_eq!(context.status, Some(200));
        assert_eq!(
            api.as_ref().map(|a| a.message.as_str()),
            Some("Market closed")
        );
    }

    /// A body this crate's model cannot read is a decode failure, and the body
    /// is what makes it diagnosable — which is exactly why it cannot be in the
    /// error a caller may log or forward.
    #[tokio::test]
    async fn a_body_that_cannot_be_decoded_stays_out_of_the_error() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            "POST /accounts/5WX00001/orders".to_string(),
            Route::ok(format!(
                r#"{{"data":{{"order":{{"id":{{"nested":"{}"}}}}}},"context":"/x"}}"#,
                sentinel::BALANCE
            )),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        #[derive(serde::Serialize, serde::Deserialize, Debug)]
        struct Order {
            order: OrderId,
        }
        #[derive(serde::Serialize, serde::Deserialize, Debug)]
        struct OrderId {
            id: u64,
        }

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::connect(&config).await.expect("authentication");
            client
                .post::<Order, _, _>(
                    "/accounts/5WX00001/orders",
                    serde_json::json!({ "order-type": "Market" }),
                )
                .await
                .map(|_| ())
        })
        .await;

        let error = result.expect_err("an object where a number belongs cannot decode");
        let TastyTradeError::Request { context, api } = &error else {
            panic!("expected a request error, got {error:?}");
        };
        assert_eq!(context.status, Some(200));
        assert!(api.is_none(), "there was no broker error document");

        // serde_json's Display renders the value it rejected. That is the whole
        // reason the raw error does not travel.
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::BALANCE),
            "the rejected body reached the error: {rendered}"
        );
        logs.assert_absent(sentinel::BALANCE, "the rejected body");
    }

    /// A decode failure must not describe the body it could not read.
    ///
    /// Two things hold this today: the log line reports the error's category
    /// and position rather than its `Display`, and the untagged
    /// `TastyApiResponse` discards the inner error anyway, so the quoted value
    /// never gets that far. This passes for either reason, which is the point —
    /// it fails when *both* are gone, and a tagged envelope alone would be
    /// enough to bring the quoted value back.
    #[tokio::test]
    async fn a_decode_failure_is_not_logged_by_rendering_the_serde_error() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            "GET /accounts/5WX00001/balances".to_string(),
            // A number where the model wants a string: serde quotes it.
            Route::ok(r#"{"data":{"cash-balance":1234567.89},"context":"/x"}"#),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        #[derive(serde::Serialize, serde::Deserialize, Debug)]
        #[serde(rename_all = "kebab-case")]
        struct Balance {
            cash_balance: String,
        }

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::connect(&config).await.expect("authentication");
            client
                .get::<Balance, _>("/accounts/5WX00001/balances")
                .await
                .map(|_| ())
        })
        .await;

        result.expect_err("a number where a string belongs cannot decode");
        logs.assert_absent("1234567.89", "the rejected balance quoted by serde");
    }

    /// The DXLink token response is the one body that *is* a credential, so a
    /// failure to decode it must say nothing about what it held — no rendered
    /// serde error, no `Json` variant handed to the caller with the rejected
    /// value inside it.
    #[tokio::test]
    async fn a_streamer_token_that_cannot_be_decoded_never_renders_the_token() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        routes.insert(
            "GET /api-quote-tokens".to_string(),
            // `token` where the model wants a string: the serde error would
            // render the rejected value, and here that value is the credential.
            Route::ok(format!(
                r#"{{"data":{{"token":["{}"],"dxlink-url":"wss://x","level":"api"}},
                     "context":"/api-quote-tokens"}}"#,
                sentinel::ACCESS_TOKEN
            )),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::connect(&config).await.expect("authentication");
            client.quote_streamer_tokens().await.map(|_| ())
        })
        .await;

        let error = result.expect_err("an array where a string belongs cannot decode");
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::ACCESS_TOKEN),
            "the streamer token reached the error: {rendered}"
        );
        assert_no_secret_leaked(&logs);
    }

    /// The verbs take a path. An absolute URL used to be concatenated onto the
    /// base URL, producing `http://host:1234http://elsewhere/...`, which does
    /// not parse — so the caller got a transport error with no status and
    /// nothing pointing at the mistake, and could not tell it from an outage.
    #[tokio::test]
    async fn an_absolute_url_is_refused_before_anything_is_sent() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::connect(&config).await.expect("authentication");
        let before = venue.requests().len();

        let error = client
            .get::<serde_json::Value, _>(format!(
                "https://api.tastyworks.com/accounts/{}/balances",
                sentinel::ACCOUNT_NUMBER
            ))
            .await
            .expect_err("an absolute URL is a caller mistake, not a request");

        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "nothing was sent, so this is a precondition: {error:?}"
        );
        assert!(
            !error.is_retryable(),
            "a mistake in the call does not improve on a retry"
        );
        assert_eq!(
            venue.requests().len(),
            before,
            "the guard must fire before anything reaches the venue"
        );
        assert!(
            !format!("{error}").contains(sentinel::ACCOUNT_NUMBER),
            "even a rejected path is redacted: {error}"
        );

        // The same guard on the verbs that mutate.
        assert!(matches!(
            client
                .delete::<serde_json::Value, _>("http://elsewhere.example/orders/1")
                .await
                .expect_err("absolute"),
            TastyTradeError::Precondition(_)
        ));
        assert!(matches!(
            client
                .post::<serde_json::Value, _, _>(
                    "https://elsewhere.example/orders",
                    serde_json::json!({}),
                )
                .await
                .expect_err("absolute"),
            TastyTradeError::Precondition(_)
        ));
    }

    /// A venue that is not there at all. This used to exit through
    /// `From<reqwest::Error>`, whose `Display` renders the URL it was trying to
    /// reach — account number included.
    #[tokio::test]
    async fn an_unreachable_venue_reports_the_verb_without_the_url() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /oauth/token".to_string(),
            Route::ok(token_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::connect(&config).await.expect("authentication");

        // The venue goes away with the session still open, which is the shape
        // of a real outage: the client is configured for a host that has
        // stopped answering. Reaching a dead port by passing an absolute URL
        // instead would test URL joining, not connectivity — and it did, until
        // review caught that the joined string never parsed at all.
        drop(venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            client
                .delete::<serde_json::Value, _>(format!(
                    "/accounts/{}/orders/17",
                    sentinel::ACCOUNT_NUMBER
                ))
                .await
                .map(|_| ())
        })
        .await;

        let error = result.expect_err("nothing is listening on that port any more");
        let TastyTradeError::Request { context, .. } = &error else {
            panic!("a transport failure must still be a typed request error, got {error:?}");
        };
        assert_eq!(
            context.status, None,
            "there is no status when nothing answered"
        );
        assert_eq!(context.method, "DELETE");
        assert!(
            error.is_retryable(),
            "a connection that never happened is the retryable case"
        );

        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::ACCOUNT_NUMBER),
            "the account number reached the error: {rendered}"
        );
        logs.assert_absent(sentinel::ACCOUNT_NUMBER, "the account number");
    }
}
