//! Cancel a running backtest.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin cancel_backtest
//! ```
//!
//! **Stops a computation**, not a position: nothing about an account changes,
//! which is why this one has no certification guard. It only touches a run that
//! has not finished.

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

    // An unrecognised status counts as still running, so this errs towards
    // trying rather than towards abandoning a run that is going.
    let Some(id) = tasty
        .backtests()
        .await?
        .into_iter()
        .find(|run| !run.is_finished())
        .and_then(|run| run.id)
    else {
        info!("No unfinished backtest to cancel.");
        return Ok(());
    };

    let cancelled = tasty.cancel_backtest(&id).await?;
    info!(
        "Cancelled {id}: now {}",
        cancelled.status.as_deref().unwrap_or("unstated")
    );

    Ok(())
}
