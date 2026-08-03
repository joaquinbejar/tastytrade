//! Edit a working order's price, through the reviewed path.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin edit_order
//! ```
//!
//! **Mutates account state**, so it refuses to run anywhere but certification.
//!
//! The same receipt machinery as a replacement, with `AmendmentIntent::Edit`
//! recorded at review time. That is what decides the verb later — an amendment
//! reviewed as a replacement cannot be applied as an edit, because the venue
//! treats them differently and a caller should not be able to swap them after
//! reading the answer.

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
        info!("This example changes a live order and runs against certification only.");
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
    // Order contents go to stdout, not through `tracing`. Underlying, price,
    // leg symbols, quantities and actions are what an account is doing with
    // its money, and INFO is the default level that reaches whatever
    // aggregator the consuming application configured.

    let live = account.live_orders().await?;
    let Some(target) = live.iter().find(|order| order.editable) else {
        println!("No editable working order to edit.");
        return Ok(());
    };

    let amendment = OrderAmendment::new(
        OrderType::Limit,
        TimeInForce::Day,
        Decimal::ZERO,
        PriceEffect::Debit,
        PriceEffect::Debit,
    )
    .with_price(target.price.unwrap_or(Decimal::from_str("1.00")?));

    let receipt = account
        .review_amendment(target.id, AmendmentIntent::Edit, &amendment)
        .await?;
    println!("Reviewed as {:?}", receipt.intent());

    for warning in receipt.warnings() {
        println!("warning: {warning}");
    }

    let edited = account.place_reviewed_amendment(receipt.accept()?).await?;

    println!("Edited: order #{} is now {}", edited.id.0, edited.status);

    Ok(())
}
