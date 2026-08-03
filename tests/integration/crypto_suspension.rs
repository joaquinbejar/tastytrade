//! The cryptocurrency order-routing suspension.
//!
//! tastytrade disabled crypto trading through the API on 2026-06-29. The
//! property worth testing is not that an error comes back — it is that
//! **nothing is sent**, on every routing path, while the read paths keep
//! working.

use std::collections::HashMap;

use rust_decimal::Decimal;
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

/// A venue that would happily accept anything, so a refusal can only be local.
async fn permissive_venue() -> MockVenue {
    let mut routes = HashMap::new();
    routes.insert(
        "POST /oauth/token".to_string(),
        Route::ok(token_response_body()),
    );
    routes.insert(
        "GET /customers/me/accounts".to_string(),
        Route::ok(one_account_body(sentinel::ACCOUNT_NUMBER)),
    );
    for suffix in [
        "/orders",
        "/orders/dry-run",
        "/complex-orders",
        "/complex-orders/dry-run",
    ] {
        routes.insert(
            format!("POST /accounts/{}{suffix}", sentinel::ACCOUNT_NUMBER),
            Route::ok(include_str!("../../Doc/order_dry_run.json")),
        );
    }
    routes.insert(
        "GET /instruments/cryptocurrencies/BTC%2FUSD".to_string(),
        Route::ok(
            r#"{"data": {"id": 1, "symbol": "BTC/USD", "instrument-type": "Cryptocurrency",
                         "short-description": "Bitcoin", "description": "Bitcoin",
                         "is-closing-only": false, "active": true,
                         "tick-size": "0.01", "streamer-symbol": "BTC/USD:CXTALP",
                         "destination-venue-symbols": []},
                "context": "/instruments/cryptocurrencies/BTC/USD"}"#,
        ),
    );
    MockVenue::start(routes).await
}

fn crypto_order() -> Order {
    OrderBuilder::default()
        .time_in_force(TimeInForce::Gtc)
        .order_type(OrderType::Limit)
        .price(Decimal::from(50000))
        .price_effect(PriceEffect::Debit)
        .legs(vec![
            OrderLegBuilder::default()
                .instrument_type(InstrumentType::Cryptocurrency)
                .symbol("BTC/USD")
                .quantity(Decimal::new(1, 1))
                .action(Action::BuyToOpen)
                .build()
                .expect("a valid leg"),
        ])
        .build()
        .expect("a valid order")
}

fn equity_order() -> Order {
    OrderBuilder::default()
        .time_in_force(TimeInForce::Day)
        .order_type(OrderType::Limit)
        .price(Decimal::ONE)
        .price_effect(PriceEffect::Debit)
        .legs(vec![
            OrderLegBuilder::default()
                .instrument_type(InstrumentType::Equity)
                .symbol("AAPL")
                .quantity(Decimal::ONE)
                .action(Action::BuyToOpen)
                .build()
                .expect("a valid leg"),
        ])
        .build()
        .expect("a valid order")
}

/// Every routing path refuses, and the venue never hears about it — even
/// though this venue would have said yes.
#[tokio::test]
async fn no_crypto_order_reaches_a_routing_path() {
    let venue = permissive_venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");
    let account = &accounts[0];

    let errors = vec![
        account.dry_run(&crypto_order()).await.err(),
        account.review_order(&crypto_order()).await.err(),
        account.place_order(&crypto_order()).await.err(),
        account
            .review_complex_order(&ComplexOrderRequest::new(
                ComplexOrderType::Oco,
                vec![crypto_order(), crypto_order()],
            ))
            .await
            .err(),
    ];

    for error in errors {
        let error = error.expect("every routing path must refuse");
        assert!(
            !error.is_retryable(),
            "nothing was sent, so nothing to retry"
        );
        let rendered = format!("{error}");
        assert!(rendered.contains("2026-06-29"), "{rendered}");
        assert!(
            rendered.contains("market data are unaffected"),
            "the message must not imply crypto data is gone: {rendered}"
        );
    }

    assert!(
        venue
            .requests()
            .iter()
            .all(|request| request.method != "POST" || request.target == "/oauth/token"),
        "nothing may have been routed: {:?}",
        venue.requests()
    );
}

/// A container is only as tradable as its least tradable component.
#[tokio::test]
async fn a_mixed_container_is_refused_for_its_crypto_component() {
    let venue = permissive_venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    let mixed =
        ComplexOrderRequest::new(ComplexOrderType::Oco, vec![equity_order(), crypto_order()]);

    assert!(accounts[0].review_complex_order(&mixed).await.is_err());

    // …and the all-equity version goes through, so the guard is about the
    // instrument and not about complex orders.
    let equities =
        ComplexOrderRequest::new(ComplexOrderType::Oco, vec![equity_order(), equity_order()]);
    assert!(accounts[0].review_complex_order(&equities).await.is_ok());
}

/// The read paths are untouched. The suspension is about routing, and a client
/// that could no longer price a cryptocurrency would be reporting the wrong
/// thing.
#[tokio::test]
async fn instrument_data_still_works() {
    let venue = permissive_venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");

    let bitcoin = client
        .get_cryptocurrency("BTC/USD")
        .await
        .expect("cryptocurrency discovery is unaffected");

    assert_eq!(bitcoin.symbol.0, "BTC/USD");
    assert!(bitcoin.active);
}

/// Everything else still routes, so the guard is narrow.
#[tokio::test]
async fn an_equity_order_is_unaffected() {
    let venue = permissive_venue().await;
    let client = TastyTrade::connect(&config_for(&venue))
        .await
        .expect("authentication must succeed");
    let accounts = client.accounts().await.expect("one account");

    assert!(accounts[0].dry_run(&equity_order()).await.is_ok());
}
