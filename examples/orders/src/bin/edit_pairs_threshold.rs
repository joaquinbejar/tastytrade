//! Change a PAIRS trade's threshold price, through the reviewed path.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin edit_pairs_threshold
//! ```
//!
//! **Mutates account state**, so it refuses to run anywhere but certification.

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

    let live = account.live_complex_orders().await?;
    let Some(id) = live
        .iter()
        .find(|container| container.complex_order_type.as_ref() == Some(&ComplexOrderType::Pairs))
        .and_then(|container| container.id.clone())
    else {
        info!("No PAIRS trade to edit.");
        return Ok(());
    };

    let edit = PairsThresholdEdit::new(
        RatioPriceComparator::GreaterOrEqual,
        Decimal::from_str("1.10")?,
    );

    let receipt = account.review_pairs_threshold(&id, &edit).await?;
    for warning in receipt.warnings() {
        info!("warning: {warning}");
    }

    let edited = account
        .place_reviewed_pairs_threshold(receipt.accept()?)
        .await?;

    info!(
        "Threshold now {:?} {:?}",
        edited.ratio_price_threshold, edited.ratio_price_comparator
    );

    Ok(())
}
