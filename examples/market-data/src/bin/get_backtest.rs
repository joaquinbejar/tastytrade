//! One backtest, with whatever progress it has made.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin get_backtest
//! ```
//!
//! Read-only.

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

    let Some(id) = tasty
        .backtests()
        .await?
        .into_iter()
        .next()
        .and_then(|run| run.id)
    else {
        info!("No backtests yet. Run create_backtest first.");
        return Ok(());
    };

    let run = tasty.backtest(&id).await?;
    info!("{id}: {}", run.status.as_deref().unwrap_or("unstated"));
    info!("  finished: {}", run.is_finished());
    info!("  trials: {}", run.trials.len());
    info!("  snapshots: {}", run.snapshots.len());
    for trial in run.trials.iter().take(10) {
        info!(
            "    {} to {}: {}",
            trial.open_date_time.as_deref().unwrap_or("-"),
            trial.close_date_time.as_deref().unwrap_or("-"),
            trial
                .profit_loss
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
    }

    Ok(())
}
