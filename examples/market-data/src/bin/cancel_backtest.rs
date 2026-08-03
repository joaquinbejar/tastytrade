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

    // The listing answers with identifiers, so finding an unfinished run means
    // fetching them. An unrecognised status counts as still running, so this
    // errs towards trying rather than towards abandoning a run that is going.
    let mut unfinished = None;
    for id in tasty.backtests().await? {
        if let Ok(run) = tasty.backtest(&id).await
            && !run.is_finished()
        {
            unfinished = Some(id);
            break;
        }
    }
    let Some(id) = unfinished else {
        info!("No unfinished backtest to cancel.");
        return Ok(());
    };

    // The venue answers 204 with no body, so there is nothing to report back
    // beyond the fact that it worked.
    tasty.cancel_backtest(&id).await?;
    info!("Cancelled {id}");

    Ok(())
}
