//! One curated watchlist by name.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin public_watchlist
//! ```
//!
//! Read-only. Curated list names contain spaces, which is the case the shared
//! path encoder exists for.

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

    let Some(first) = tasty.public_watchlists(false).await?.into_iter().next() else {
        info!("The venue published no curated lists.");
        return Ok(());
    };

    let list = tasty.public_watchlist(&first.name).await?;
    info!(
        "{}: {} entr(y/ies)",
        list.name,
        list.watchlist_entries.len()
    );
    for entry in list.watchlist_entries.iter().take(20) {
        info!(
            "  {} ({})",
            entry.symbol.0,
            entry.instrument_type.as_deref().unwrap_or("unstated")
        );
    }

    Ok(())
}
