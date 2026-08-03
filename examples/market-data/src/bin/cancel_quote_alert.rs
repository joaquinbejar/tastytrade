//! Cancel a quote alert.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin cancel_quote_alert
//! ```
//!
//! **Mutates user state**, so it refuses to run anywhere but certification.

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
        info!("This example cancels an alert and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    // Only an alert that has not fired: cancelling one that already triggered
    // is not what this demonstrates.
    let Some(id) = tasty
        .quote_alerts()
        .await?
        .into_iter()
        .find(|alert| alert.triggered_at.is_none())
        .and_then(|alert| alert.alert_external_id)
    else {
        info!("No pending alert to cancel. Run create_quote_alert first.");
        return Ok(());
    };

    let cancelled = tasty.cancel_quote_alert(&id).await?;
    info!(
        "Cancelled {id}: {} {} {}",
        cancelled
            .symbol
            .as_ref()
            .map(|symbol| symbol.0.as_str())
            .unwrap_or("-"),
        cancelled
            .operator
            .as_ref()
            .map(QuoteAlertOperator::as_wire)
            .unwrap_or("-"),
        cancelled.threshold.as_deref().unwrap_or("-")
    );

    Ok(())
}
