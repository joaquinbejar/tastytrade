//! Preview an OCO without routing it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin dry_run_complex_order
//! ```
//!
//! Stops at the receipt and places nothing. `POST /complex-orders/dry-run` is
//! reachable only through `review_complex_order`, which hands back the receipt
//! that `place_reviewed_complex_order` needs — so the answer cannot be skipped.

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
        info!("This example runs against certification only. Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let accounts = tasty.accounts().await?;
    let Some(account) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    info!("Account {} (CERTIFICATION)", account.number().redacted());
    // Component symbols, buying-power effects, order identifiers and the
    // venue's warning prose go to stdout. A warning can name the account or
    // the buying power it is about, and INFO is the default level that reaches
    // whatever aggregator the consuming application configured.

    // OCO: take profit or stop out, whichever comes first. Two components,
    // which is what makes it an OCO — one would be refused locally.
    let request = ComplexOrderRequest::new(
        ComplexOrderType::Oco,
        vec![
            limit_order(Action::SellToClose, "200.00")?,
            limit_order(Action::SellToClose, "150.00")?,
        ],
    );

    let receipt = account.review_complex_order(&request).await?;

    println!(
        "Buying power effect: {:?}",
        receipt.result().buying_power_effect
    );
    for warning in receipt.warnings() {
        // Venue prose written for a person: it can name the account or the
        // buying power, so it goes on screen rather than into a log.
        println!("  warning: {warning}");
    }
    println!("Clean: {} — nothing was placed.", receipt.is_clean());

    // A one-sided OCO never reaches the venue.
    let one_sided = ComplexOrderRequest::new(
        ComplexOrderType::Oco,
        vec![limit_order(Action::SellToClose, "200.00")?],
    );
    match account.review_complex_order(&one_sided).await {
        Ok(_) => println!("a one-sided OCO was accepted, which is a bug"),
        Err(error) => println!(
            "one-sided OCO refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    Ok(())
}

fn limit_order(action: Action, price: &str) -> Result<Order, Box<dyn std::error::Error>> {
    Ok(OrderBuilder::default()
        .time_in_force(TimeInForce::Gtc)
        .order_type(OrderType::Limit)
        .price(Decimal::from_str(price)?)
        .price_effect(PriceEffect::Credit)
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
