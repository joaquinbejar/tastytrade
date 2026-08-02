//! Cross-module REST invariants, driven against a loopback venue.
//!
//! These are the properties unit tests cannot reach: they need the real
//! reqwest client, the real envelope decoding and the real error mapping,
//! wired end to end.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::{Environment, TastyTradeError};
use tracing::Level;

use crate::support::{
    CapturedLogs, MockVenue, Route, capture_logs_at, login_response_body,
    partially_unparseable_accounts_body, sentinel, wholly_unparseable_accounts_body,
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
        Route::ok(partially_unparseable_accounts_body()),
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
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(account_body),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    // TRACE: if a body survives anywhere, it survives here.
    let (count, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::login(&config)
            .await
            .expect("login must succeed");
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
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    // A well-formed envelope that simply carries no pagination block, which
    // used to reach an expect() and abort the caller's process.
    routes.insert(
        "GET /instruments/equities/active".to_string(),
        Route::ok(r#"{"data":{"items":[]},"context":"/instruments/equities/active"}"#),
    );
    let venue = MockVenue::start(routes).await;
    let config = config_for(&venue);

    let client = TastyTrade::login(&config)
        .await
        .expect("login must succeed");

    let error = client
        .list_active_equities(0)
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
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
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

    let client = TastyTrade::login(&config)
        .await
        .expect("login must succeed");

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
        "POST /sessions".to_string(),
        Route::ok(login_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(wholly_unparseable_accounts_body()),
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
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
        let client = TastyTrade::login(&config).await.expect("login");
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
        let client = TastyTrade::login(&config).await.expect("login");
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
        let client = TastyTrade::login(&config).await.expect("login");
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
}
