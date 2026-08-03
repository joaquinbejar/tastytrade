//! Place an OTOCO through the reviewed path.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin place_otoco
//! ```
//!
//! **Routes real money**, so it refuses to run anywhere but certification.
//!
//! OTOCO is "one triggers a one-cancels-other pair": an entry that, once
//! filled, arms a take-profit and a stop that cancel each other. The payload
//! differs from an OCO in that the first component is the trigger.

use rust_decimal::Decimal;
use std::str::FromStr;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    if config.environment() != Environment::Certification {
        info!("This example routes an order and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let accounts = tasty.accounts().await?;
    let Some(account) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    info!("Account {} (CERTIFICATION)", account.number().redacted());

    let request = ComplexOrderRequest::new(
        ComplexOrderType::Otoco,
        vec![
            // The trigger: an entry.
            order(Action::BuyToOpen, "1.00", PriceEffect::Debit)?,
            // Armed once it fills: take profit, or stop out.
            order(Action::SellToClose, "2.00", PriceEffect::Credit)?,
            order(Action::SellToClose, "0.50", PriceEffect::Credit)?,
        ],
    );

    let receipt = account.review_complex_order(&request).await?;
    for warning in receipt.warnings() {
        info!("warning: {warning}");
    }

    // `accept()` refuses when the venue attached warnings — not a refusal to
    // proceed, a refusal to proceed silently.
    let placed = account
        .place_reviewed_complex_order(receipt.accept()?)
        .await?;

    info!(
        "Placed {} with {} component(s)",
        placed
            .id
            .as_ref()
            .map(|id| id.0.as_str())
            .unwrap_or("(no id)"),
        placed.orders.len()
    );

    Ok(())
}

fn order(
    action: Action,
    price: &str,
    effect: PriceEffect,
) -> Result<Order, Box<dyn std::error::Error>> {
    Ok(OrderBuilder::default()
        .time_in_force(TimeInForce::Gtc)
        .order_type(OrderType::Limit)
        .price(Decimal::from_str(price)?)
        .price_effect(effect)
        .legs(vec![
            OrderLegBuilder::default()
                .instrument_type(InstrumentType::Equity)
                .symbol("AAPL")
                .quantity(Decimal::ONE)
                .action(action)
                .build()?,
        ])
        .build()?)
}
