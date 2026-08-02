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
            let client = TastyTrade::login(&config).await.expect("login");
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
        let client = TastyTrade::login(&config).await.expect("login");
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
/// endpoint and an environment. POST, DELETE and login never inspected the
/// status at all: they handed the body straight to serde and surfaced whatever
/// it said about a document it could not decode. These pin the shared path.
mod every_verb_reports_failures_alike {
    use super::*;

    /// A `POST /sessions` that fails, plus the routes a test needs afterwards.
    async fn venue_with_login(login: Route) -> MockVenue {
        let mut routes = HashMap::new();
        routes.insert("POST /sessions".to_string(), login);
        MockVenue::start(routes).await
    }

    /// The login endpoint is the one request whose *request* body is a
    /// credential, and a venue that echoes it back is not hypothetical: error
    /// documents routinely quote the field that failed validation.
    #[tokio::test]
    async fn rejected_credentials_are_an_auth_error_not_a_decode_failure() {
        let venue = venue_with_login(Route::status(
            401,
            format!(
                r#"{{"error":{{"code":"invalid_credentials","message":"Invalid login",
                     "errors":[{{"code":"password","message":"{} is not correct"}}]}}}}"#,
                sentinel::PASSWORD
            ),
        ))
        .await;
        let config = config_for(&venue);

        let (result, logs) =
            capture_logs_at(Level::TRACE, async { TastyTrade::login(&config).await }).await;

        let error = result.expect_err("a 401 must not produce a session");

        // Auth rather than Request: the credentials are wrong, so retrying with
        // the same ones is pointless, and `BackoffPolicy` already treats Auth as
        // terminal.
        let TastyTradeError::Auth(message) = &error else {
            panic!("a rejected login must be an auth error, got {error:?}");
        };
        assert!(
            message.contains("Invalid login"),
            "the venue's own summary is what a caller acts on: {message}"
        );
        assert!(
            !error.is_retryable(),
            "wrong credentials do not become right on a retry"
        );

        // The nested detail quoted the password. Neither the error nor the log
        // may carry it.
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::PASSWORD),
            "the password reached the error: {rendered}"
        );
        assert_no_secret_leaked(&logs);
    }

    /// A login endpoint that is down says nothing about the credentials, so
    /// re-labelling every failure as `Auth` would be wrong.
    #[tokio::test]
    async fn an_unavailable_login_endpoint_stays_a_request_error() {
        let venue = venue_with_login(Route::status(503, "<html>maintenance</html>")).await;
        let config = config_for(&venue);

        let error = TastyTrade::login(&config)
            .await
            .expect_err("a 503 must not produce a session");

        let TastyTradeError::Request { context, api } = &error else {
            panic!("a 503 on /sessions is a request failure, got {error:?}");
        };
        assert_eq!(context.status, Some(503));
        assert_eq!(context.method, "POST");
        assert_eq!(context.operation, "/sessions");
        assert!(api.is_none(), "an HTML body is not a broker error document");
        assert!(
            error.is_retryable(),
            "a venue that is temporarily down is exactly the retryable case"
        );
    }

    /// The verb that can place an order.
    #[tokio::test]
    async fn a_rejected_post_carries_the_status_and_a_redacted_endpoint() {
        let mut routes = HashMap::new();
        routes.insert(
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
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
            let client = TastyTrade::login(&config)
                .await
                .expect("login must succeed");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::login(&config)
                .await
                .expect("login must succeed");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
        );
        routes.insert(
            "POST /accounts/5WX00001/orders".to_string(),
            Route::ok(r#"{"error":{"code":"preflight","message":"Market closed"}}"#),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::login(&config).await.expect("login");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
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
            let client = TastyTrade::login(&config).await.expect("login");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
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
            let client = TastyTrade::login(&config).await.expect("login");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
        );
        routes.insert(
            "GET /api-quote-tokens".to_string(),
            // `token` where the model wants a string: the serde error would
            // render the rejected value, and here that value is the credential.
            Route::ok(format!(
                r#"{{"data":{{"token":["{}"],"dxlink-url":"wss://x","level":"api"}},
                     "context":"/api-quote-tokens"}}"#,
                sentinel::SESSION_TOKEN
            )),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let (result, logs) = capture_logs_at(Level::TRACE, async {
            let client = TastyTrade::login(&config).await.expect("login");
            client.quote_streamer_tokens().await.map(|_| ())
        })
        .await;

        let error = result.expect_err("an array where a string belongs cannot decode");
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(sentinel::SESSION_TOKEN),
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::login(&config).await.expect("login");
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
            "POST /sessions".to_string(),
            Route::ok(login_response_body()),
        );
        let venue = MockVenue::start(routes).await;
        let config = config_for(&venue);

        let client = TastyTrade::login(&config).await.expect("login");

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
