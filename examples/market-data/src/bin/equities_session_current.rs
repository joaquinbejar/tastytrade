//! The equities session in progress.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin equities_session_current
//! ```

use chrono::Utc;
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

    let session = tasty.current_equities_session(None).await?;

    info!("State: {}", session.state.as_deref().unwrap_or("unstated"));
    info!(
        "Open {} close {} (extended {} to {})",
        session
            .open_at
            .map(|at| at.to_string())
            .unwrap_or_else(|| "-".to_string()),
        session
            .close_at
            .map(|at| at.to_string())
            .unwrap_or_else(|| "-".to_string()),
        session
            .start_at
            .map(|at| at.to_string())
            .unwrap_or_else(|| "-".to_string()),
        session
            .close_at_ext
            .map(|at| at.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    info!(
        "Open now: {:?}",
        session.is_open_at(Utc::now().fixed_offset())
    );

    Ok(())
}
