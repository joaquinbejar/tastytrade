//! One of this user's watchlists.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin get_watchlist
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

    let Some(first) = tasty.watchlists().await?.into_iter().next() else {
        info!("This user has no watchlists. Run create_watchlist first.");
        return Ok(());
    };

    let list = tasty.watchlist(&first.name).await?;
    info!("{}: order index {:?}", list.name, list.order_index);
    for entry in &list.watchlist_entries {
        info!(
            "  {} ({})",
            entry.symbol.0,
            entry.instrument_type.as_deref().unwrap_or("unstated")
        );
    }

    Ok(())
}
