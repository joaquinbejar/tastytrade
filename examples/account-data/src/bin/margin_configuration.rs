//! The venue's public margin configuration.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin margin_configuration
//! ```
//!
//! Public and read-only, but it goes through the authenticated client like
//! everything else: one transport, one error shape, one place the status is
//! checked.

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

    let tasty = TastyTrade::connect(&config).await?;
    let configuration = tasty.margin_requirements_configuration().await?;

    match configuration.risk_free_rate {
        // A ratio, so `Decimal` and not `f64` — it multiplies money.
        Some(rate) => info!("Risk-free rate: {rate}"),
        None => info!("The venue reported no risk-free rate"),
    }

    Ok(())
}
