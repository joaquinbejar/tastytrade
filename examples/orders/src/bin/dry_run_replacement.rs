//! Preview an amendment to a working order, without applying it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin dry_run_replacement
//! ```
//!
//! `POST /orders/{id}/dry-run` exists so a replacement can be previewed without
//! routing. It is not exposed as a bare method — the only way to reach it is
//! `review_amendment`, which hands back a receipt. That receipt is the sole way
//! to apply the amendment afterwards, so a caller cannot skip the answer.
//!
//! This example **stops at the receipt** and applies nothing. Cert only.

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
    // A dry run routes nothing, but the order it names is real, so this stays
    // on certification like everything else that touches a working order.
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
    info!("Account {}", account.number().redacted());

    let live = account.live_orders().await?;
    let Some(target) = live.iter().find(|order| order.editable) else {
        info!("No editable working order to preview an amendment against.");
        return Ok(());
    };
    info!("Previewing an amendment to order #{}", target.id.0);

    let amendment = OrderAmendment::new(
        OrderType::Limit,
        TimeInForce::Day,
        Decimal::ZERO,
        PriceEffect::Debit,
        PriceEffect::Debit,
    )
    .with_price(Decimal::from_str("1.00")?);

    let receipt = account
        .review_amendment(target.id, AmendmentIntent::Replace, &amendment)
        .await?;

    info!(
        "Buying power effect: {:?}",
        receipt.result().buying_power_effect
    );
    for warning in receipt.warnings() {
        // Venue prose written for a person: it can name the account or the
        // buying power, so it goes on screen rather than into a log
        // aggregator.
        info!("  warning: {warning}");
    }
    info!(
        "Clean: {} — the receipt is what `place_reviewed_amendment` needs, and \
         this example stops here.",
        receipt.is_clean()
    );

    Ok(())
}
