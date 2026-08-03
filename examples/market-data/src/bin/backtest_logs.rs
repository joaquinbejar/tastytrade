//! A backtest's logs.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin backtest_logs
//! ```
//!
//! Read-only. The logs come back as the venue's own JSON: no schema is
//! published for them, and a type invented for a log is a type that stops
//! decoding the first time a field is added.

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

    let Some(id) = tasty.backtests().await?.into_iter().next() else {
        info!("No backtests yet. Run create_backtest first.");
        return Ok(());
    };

    let logs = tasty.backtest_logs(&id).await?;
    info!("Logs for {id}:");
    info!("{logs}");

    Ok(())
}
