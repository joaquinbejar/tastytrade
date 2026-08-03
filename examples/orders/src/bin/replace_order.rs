//! Replace a working order, through the reviewed path.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin replace_order
//! ```
//!
//! **Mutates account state**, so it refuses to run anywhere but certification.
//!
//! Cancel-replace is how a resting order is repriced. Doing it as cancel then
//! place loses queue position and leaves the account unhedged in between; this
//! is one request. It is **not atomic at the venue** — a fill on the original
//! aborts the replacement — and this crate does not paper over that.
//!
//! The path is `review_amendment` → read the warnings → `accept()` →
//! `place_reviewed_amendment`. The receipt is bound to the account **and** the
//! deployment, because certification reuses production account numbering.

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
        println!("No editable working order to replace.");
        return Ok(());
    };

    // Reprice one cent below whatever it is now, or a nominal price if the
    // order type carries none.
    let price = target.price.unwrap_or(Decimal::from_str("1.00")?) - Decimal::from_str("0.01")?;

    let amendment = OrderAmendment::new(
        target.order_type,
        target.time_in_force,
        Decimal::ZERO,
        target.price_effect.unwrap_or(PriceEffect::Debit),
        PriceEffect::Debit,
    )
    .with_price(price);

    let receipt = account
        .review_amendment(target.id, AmendmentIntent::Replace, &amendment)
        .await?;

    for warning in receipt.warnings() {
        println!("warning: {warning}");
    }

    // `accept()` refuses when the venue attached warnings — not a refusal to
    // proceed, a refusal to proceed silently. `accept_with_warnings()` is the
    // deliberate alternative, and its name says so.
    let reviewed = receipt.accept()?;
    let replaced = account.place_reviewed_amendment(reviewed).await?;

    println!(
        "Replaced: order #{} is now {}",
        replaced.id.0, replaced.status
    );

    Ok(())
}
