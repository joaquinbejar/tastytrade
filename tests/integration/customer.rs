//! The customer resource and the single-account endpoint.
//!
//! Two properties. The first is that a customer's personal data reaches no log
//! line, `Debug` rendering or error message on the way through — this is the
//! most sensitive object in the API and the only test that matters is the one
//! that watches it travel. The second is that `Ok(None)` from `account()` now
//! means the venue said 404, and nothing else.

use std::collections::HashMap;

use tastytrade::TastyTrade;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tracing::Level;

use crate::support::{
    CapturedLogs, MockVenue, Route, capture_logs_at, partially_unparseable_accounts_body, sentinel,
    token_response_body,
};

/// Personal data the venue might send. Distinctive enough that a substring
/// search cannot produce a false positive.
mod pii {
    pub const FIRST_NAME: &str = "SENTINEL-firstname-Rowan";
    pub const LAST_NAME: &str = "SENTINEL-lastname-Okonkwo";
    pub const TAX_NUMBER: &str = "SENTINEL-tax-000112222";
    pub const EMAIL: &str = "sentinel-person@example.invalid";
    pub const STREET: &str = "SENTINEL-street-12-Nowhere-Lane";
    pub const BIRTH_DATE: &str = "1970-02-03";
    pub const NET_WORTH: &str = "987654321";
}

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

/// A customer body carrying a sentinel in every kind of field the resource has.
fn customer_body() -> String {
    format!(
        r#"{{
            "data": {{
                "id": "SENTINEL-customer-id-77",
                "first-name": "{}",
                "last-name": "{}",
                "email": "{}",
                "tax-number": "{}",
                "tax-number-type": "SSN",
                "birth-date": "{}",
                "is-professional": false,
                "agreed-to-margining": true,
                "address": {{
                    "street-one": "{}",
                    "city": "SENTINEL-city-Nowhere",
                    "postal-code": "SENTINEL-00000",
                    "country": "USA"
                }},
                "customer-suitability": {{
                    "net-worth": {},
                    "annual-net-income": 123456,
                    "employer-name": "SENTINEL-employer-Acme",
                    "occupation": "SENTINEL-occupation-Cooper"
                }},
                "person": {{
                    "first-name": "{}",
                    "last-name": "{}",
                    "birth-date": "{}"
                }}
            }},
            "context": "/customers/me"
        }}"#,
        pii::FIRST_NAME,
        pii::LAST_NAME,
        pii::EMAIL,
        pii::TAX_NUMBER,
        pii::BIRTH_DATE,
        pii::STREET,
        pii::NET_WORTH,
        pii::FIRST_NAME,
        pii::LAST_NAME,
        pii::BIRTH_DATE,
    )
}

fn account_body(number: &str) -> String {
    format!(
        r#"{{
            "data": {{
                "account-number": "{number}",
                "nickname": "Test",
                "account-type-name": "Individual",
                "margin-or-cash": "Margin",
                "opened-at": "2025-01-14T10:22:41.000+00:00"
            }},
            "context": "/customers/me/accounts/{number}"
        }}"#
    )
}

async fn venue_with(extra: Vec<(&str, Route)>) -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    for (key, route) in extra {
        routes.insert(key.to_string(), route);
    }
    MockVenue::start(routes).await
}

fn assert_no_pii(logs: &CapturedLogs, rendered: &str) {
    for (value, what) in [
        (pii::FIRST_NAME, "the first name"),
        (pii::LAST_NAME, "the last name"),
        (pii::TAX_NUMBER, "the tax number"),
        (pii::EMAIL, "the email address"),
        (pii::STREET, "the street address"),
        (pii::NET_WORTH, "the net worth"),
    ] {
        logs.assert_absent(value, what);
        assert!(
            !rendered.contains(value),
            "{what} appeared in a rendering: {rendered}"
        );
    }
}

/// The one that matters. The customer travels from the socket through the
/// decoder into a value the caller holds, at `TRACE`, and must appear at no
/// point in between — including in its own `Debug`.
#[tokio::test]
async fn a_customer_reaches_no_log_line_and_renders_nothing() {
    let venue = venue_with(vec![("GET /customers/me", Route::ok(customer_body()))]).await;
    let config = config_for(&venue);

    let (customer, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config).await?;
        client.customer().await
    })
    .await;

    let customer = customer.expect("the customer must decode");

    // `{:#?}` as well as `{:?}`: the alternate form is what a panic message
    // and most error reports use.
    let rendered = format!("{customer:?} {customer} {customer:#?}");
    assert_no_pii(&logs, &rendered);
    assert!(
        rendered.contains("redacted"),
        "the rendering must say it is redacted: {rendered}"
    );

    // …and the data is all there for a caller who names the field.
    assert_eq!(customer.first_name.as_deref(), Some(pii::FIRST_NAME));
    assert_eq!(customer.tax_number.as_deref(), Some(pii::TAX_NUMBER));
    assert_eq!(
        customer
            .address
            .as_ref()
            .and_then(|address| address.street_one.as_deref()),
        Some(pii::STREET)
    );
    assert_eq!(customer.is_professional, Some(false));
}

/// The nested sections redact too. A hand-written `Debug` on the outer type
/// only would have let the inner ones print themselves.
#[tokio::test]
async fn every_nested_section_redacts_as_well() {
    let venue = venue_with(vec![("GET /customers/me", Route::ok(customer_body()))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let customer = client.customer().await.expect("the customer must decode");
    let address = customer.address.as_ref().expect("an address");
    let suitability = customer
        .customer_suitability
        .as_ref()
        .expect("a suitability block");
    let person = customer.person.as_ref().expect("a person");

    let rendered = format!("{address:?} {address:#?} {suitability:?} {person:?} {person:#?}");

    for value in [
        pii::STREET,
        pii::NET_WORTH,
        pii::FIRST_NAME,
        pii::LAST_NAME,
        "Acme",
        "Cooper",
    ] {
        assert!(!rendered.contains(value), "{value} leaked: {rendered}");
    }

    // The count is what `Debug` may say, and it has to be true.
    assert_eq!(address.populated_fields(), 4);
}

/// An error carrying a customer must not render it either — an error message
/// is a string the caller prints.
#[tokio::test]
async fn a_customer_inside_an_error_stays_redacted() {
    let venue = venue_with(vec![("GET /customers/me", Route::ok(customer_body()))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let customer = client.customer().await.expect("the customer must decode");

    let error = TastyTradeError::Precondition(format!("cannot act for {customer}"));

    let rendered = format!("{error} {error:?}");
    assert!(!rendered.contains(pii::FIRST_NAME), "{rendered}");
    assert!(!rendered.contains(pii::TAX_NUMBER), "{rendered}");
}

/// `allow-missing` is the venue's own way of turning a 404 into an ordinary
/// answer, and the query must actually be sent.
#[tokio::test]
async fn find_customer_sends_allow_missing_and_maps_an_empty_answer_to_none() {
    let venue = venue_with(vec![(
        "GET /customers/12345",
        Route::ok(r#"{"data": {}, "context": "/customers/12345"}"#),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let found = client
        .find_customer("12345")
        .await
        .expect("a missing customer is an ordinary answer");

    assert!(found.is_none(), "a customer with no id is not a customer");

    let target = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/customers/"))
        .expect("the customer request must have been sent")
        .target;
    assert_eq!(target, "/customers/12345?allow-missing=true");
}

/// …and a customer that *is* there still comes back.
#[tokio::test]
async fn find_customer_returns_the_customer_when_there_is_one() {
    let venue = venue_with(vec![("GET /customers/me", Route::ok(customer_body()))]).await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let found = client.find_customer("me").await.expect("a customer");

    assert!(found.is_some());
}

/// The regression `account()` was rewritten for.
///
/// The listing carries one account that decodes and one that does not.
/// `Items<T>` skips the broken one — deliberately — but the old `account()`
/// filtered that same listing, so asking for the *healthy* account still
/// worked while asking for the broken one returned `Ok(None)`: identical to
/// "this session cannot see it". Now the lookup never reads the listing.
#[tokio::test]
async fn a_single_account_lookup_does_not_depend_on_its_siblings() {
    let venue = venue_with(vec![
        (
            "GET /customers/me/accounts",
            Route::ok(partially_unparseable_accounts_body()),
        ),
        (
            "GET /customers/me/accounts/SENTINEL-5WX00042",
            Route::ok(account_body(sentinel::ACCOUNT_NUMBER)),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    // The listing drops it: this is the state the old lookup inherited.
    let listed = client.accounts().await.expect("one account survives");
    assert_eq!(listed.len(), 1);
    assert_ne!(listed[0].number().0, sentinel::ACCOUNT_NUMBER);

    // The single-account endpoint finds it anyway.
    let account = client
        .account(sentinel::ACCOUNT_NUMBER)
        .await
        .expect("the request must succeed")
        .expect("the account exists and the venue says so");

    assert_eq!(account.number().0, sentinel::ACCOUNT_NUMBER);
    assert_eq!(account.details().nickname, "Test");

    // The two endpoints answer with different objects, and the difference is
    // visible rather than flattened. The listing decorates each account with
    // an authority level; the single fetch returns the account itself, so
    // there is nothing to report and `None` says so. An empty string here
    // would have read as a level the venue supplied and left blank.
    assert_eq!(listed[0].authority_level(), Some("owner"));
    assert_eq!(account.authority_level(), None);
}

/// `Ok(None)` means 404, and only 404.
#[tokio::test]
async fn only_a_404_becomes_ok_none() {
    let venue = venue_with(vec![
        (
            "GET /customers/me/accounts/GONE",
            Route::status(404, r#"{"error":{"code":"not_found","message":"nope"}}"#),
        ),
        (
            "GET /customers/me/accounts/BROKEN",
            Route::status(500, r#"{"error":{"code":"oops","message":"server"}}"#),
        ),
    ])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    assert!(
        client
            .account_by_number("GONE")
            .await
            .expect("a 404 is an answer")
            .is_none()
    );

    // A 500 is a failure, not an absence. Collapsing it to `Ok(None)` would
    // tell a caller the account does not exist because the venue was down.
    //
    // `match` rather than `expect_err`: `Account` deliberately has no `Debug`,
    // because deriving one would print the account number.
    match client.account_by_number("BROKEN").await {
        Ok(_) => panic!("a 500 must not be reported as a missing account"),
        Err(error) => assert!(error.is_retryable(), "a server error is worth retrying"),
    }
}

/// The account number is a path segment and stays redacted in the error.
#[tokio::test]
async fn the_single_account_path_encodes_and_redacts_the_number() {
    // A 500 rather than letting the venue 404: a 404 is `Ok(None)` by design,
    // and this test needs an error value to inspect.
    let venue = venue_with(vec![(
        "GET /customers/me/accounts/5WX%2F00042",
        Route::status(500, r#"{"error":{"code":"oops","message":"server"}}"#),
    )])
    .await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let error = match client.account_by_number("5WX/00042").await {
        Ok(_) => panic!("a 500 must surface as an error"),
        Err(error) => error,
    };

    let target = venue
        .requests()
        .into_iter()
        .rfind(|request| request.target.starts_with("/customers/"))
        .expect("the request must have been sent")
        .target;
    assert_eq!(target, "/customers/me/accounts/5WX%2F00042");

    let rendered = format!("{error}");
    assert!(
        !rendered.contains("00042"),
        "the account number reached the error: {rendered}"
    );
}

/// The customer identifier gets the account number's treatment.
///
/// It is the other identifier this crate puts in a path, and it reached the
/// same three places the account number was kept out of: the request context
/// carried in every `TastyTradeError::Request`, the `Display` a caller prints,
/// and the DEBUG line the client writes about the request it is making. A
/// failing response is the case that matters, because that is when the value
/// is rendered rather than merely held.
#[tokio::test]
async fn a_customer_identifier_stays_out_of_the_error_and_the_logs() {
    const CUSTOMER_ID: &str = "SENTINEL-customer-78a1f0c2";

    let venue = venue_with(vec![(
        &format!("GET /customers/{CUSTOMER_ID}"),
        Route::status(500, r#"{"error":{"code":"oops","message":"server"}}"#),
    )])
    .await;

    let (errors, logs) = capture_logs_at(Level::TRACE, async {
        let client = TastyTrade::connect(&config_for(&venue))
            .await
            .expect("authentication must succeed");
        // Both methods that take an identifier. `find_customer` adds a query
        // parameter, which is where the redaction used to swallow the rest of
        // the path along with the value.
        let by_id = client
            .customer_by_id(CUSTOMER_ID)
            .await
            .expect_err("a 500 must surface as an error");
        let found = client
            .find_customer(CUSTOMER_ID)
            .await
            .expect_err("a 500 must surface as an error");
        (by_id, found)
    })
    .await;
    let error = errors.0;

    // Both were really sent, so this is not passing because nothing happened.
    let targets: Vec<String> = venue
        .requests()
        .into_iter()
        .filter(|request| request.target.starts_with("/customers/"))
        .map(|request| request.target)
        .collect();
    assert_eq!(
        targets,
        vec![
            format!("/customers/{CUSTOMER_ID}"),
            format!("/customers/{CUSTOMER_ID}?allow-missing=true"),
        ]
    );

    let rendered = format!("{error} {error:?} {} {:?}", errors.1, errors.1);
    assert!(
        !rendered.contains(CUSTOMER_ID),
        "the customer identifier reached the error: {rendered}"
    );
    assert!(
        rendered.contains("{customer}"),
        "the error lost the path it was reaching for: {rendered}"
    );
    // `find_customer` sends `allow-missing`, and the query has to survive the
    // redaction: an error that cannot say which request failed is worse than
    // one that says too much.
    assert!(
        rendered.contains("allow-missing"),
        "the redaction ate the query string: {rendered}"
    );

    let captured = logs.contents();
    assert!(
        !captured.contains(CUSTOMER_ID),
        "the customer identifier reached a log line"
    );
}
