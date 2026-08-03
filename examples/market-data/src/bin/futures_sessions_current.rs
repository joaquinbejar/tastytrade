//! The current session for every futures collection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin futures_sessions_current
//! ```

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

    for session in tasty.current_futures_sessions().await? {
        info!(
            "{}: {} (open {})",
            session
                .instrument_collection
                .as_ref()
                .map(InstrumentCollection::as_wire)
                .unwrap_or("-"),
            session.state.as_deref().unwrap_or("unstated"),
            session
                .open_at
                .map(|at| at.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
    }

    Ok(())
}
