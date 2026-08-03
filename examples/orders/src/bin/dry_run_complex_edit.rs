//! Preview a PAIRS threshold change, without applying it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin dry_run_complex_edit
//! ```
//!
//! Stops at the receipt. `PATCH /complex-orders/{id}` changes the threshold
//! price of a PAIRS trade and nothing else — narrower than the plain-order
//! patch, which is why it has its own type rather than a generic edit that
//! would advertise fields the route ignores.

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

    let live = account.live_complex_orders().await?;
    let Some(pairs) = live
        .iter()
        .find(|container| container.complex_order_type.as_ref() == Some(&ComplexOrderType::Pairs))
    else {
        info!("No PAIRS trade to preview a threshold change against.");
        return Ok(());
    };
    let Some(id) = pairs.id.clone() else {
        info!("The venue returned a PAIRS trade with no id.");
        return Ok(());
    };

    let edit = PairsThresholdEdit::new(
        RatioPriceComparator::LessOrEqual,
        Decimal::from_str("1.25")?,
    );

    let receipt = account.review_pairs_threshold(&id, &edit).await?;

    for warning in receipt.warnings() {
        info!("warning: {warning}");
    }
    info!(
        "Clean: {} — the receipt is what `place_reviewed_pairs_threshold` needs, \
         and this example stops here.",
        receipt.is_clean()
    );

    Ok(())
}
