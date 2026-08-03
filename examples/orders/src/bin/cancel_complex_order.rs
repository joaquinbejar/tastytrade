//! Cancel a complex order.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin cancel_complex_order
//! ```
//!
//! **Mutates account state**, so it refuses to run anywhere but certification.
//!
//! Cancelling the container requests cancellation of every component that is
//! not already terminal. A component that has filled stays filled — this is a
//! request, not an undo.

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
        info!("This example cancels a live order and runs against certification only.");
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
    let Some(target) = live
        .iter()
        .find(|container| container.has_working_components())
    else {
        info!("No complex order with working components to cancel.");
        return Ok(());
    };
    let Some(id) = target.id.clone() else {
        info!("The venue returned a complex order with no id.");
        return Ok(());
    };

    let cancelled = account.cancel_complex_order(&id).await?;

    info!("Requested cancellation of {}", id.0);
    for component in &cancelled.orders {
        info!(
            "  {} is now {}",
            component.id.as_deref().unwrap_or("-"),
            component
                .status
                .as_ref()
                .map(OrderStatus::as_wire)
                .unwrap_or("-")
        );
    }

    Ok(())
}
