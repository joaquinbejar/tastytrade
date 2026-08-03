//! This user's own watchlists.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin list_watchlists
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

    let lists = tasty.watchlists().await?;
    info!("{} list(s)", lists.len());
    for list in &lists {
        info!(
            "  {}: {} entr(y/ies)",
            list.name,
            list.watchlist_entries.len()
        );
    }

    Ok(())
}
