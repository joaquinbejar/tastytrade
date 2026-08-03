//! Which date ranges the backtester holds data for.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin available_dates
//! ```
//!
//! Read-only, and the cheapest probe of whether the area is reachable at all.

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

    match tasty.available_dates().await {
        Ok(ranges) => {
            info!(
                "{} symbol range(s) from {BACKTESTER_BASE_URL}",
                ranges.len()
            );
            for range in ranges.iter().take(20) {
                info!(
                    "  {}: {} to {}",
                    range.symbol.as_deref().unwrap_or("-"),
                    range.start_date.as_deref().unwrap_or("-"),
                    range.end_date.as_deref().unwrap_or("-")
                );
            }
        }
        Err(error) => info!("Backtesting did not answer: {error}"),
    }

    Ok(())
}
