//! Every backtest this user has run.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin list_backtests
//! ```
//!
//! Read-only. Backtesting is served by its **own host**, published in its own
//! OpenAPI document — one host, with no sandbox counterpart — so the session's
//! environment decides which credentials are used but not which service is
//! reached.

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
    info!(
        "Session: {} — backtests go to {BACKTESTER_BASE_URL}",
        config.environment()
    );

    let tasty = TastyTrade::connect(&config).await?;

    match tasty.backtests().await {
        Ok(runs) => {
            info!("{} backtest(s)", runs.len());
            for run in runs.iter().take(20) {
                info!(
                    "  {} {} — {} ({}%)",
                    run.id.as_deref().unwrap_or("-"),
                    run.symbol.as_deref().unwrap_or("-"),
                    run.status.as_deref().unwrap_or("unstated"),
                    run.progress
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }
        // Whether the area is still publicly available is exactly what running
        // this establishes, so the failure is reported rather than swallowed.
        Err(error) => info!("Backtesting did not answer: {error}"),
    }

    Ok(())
}
