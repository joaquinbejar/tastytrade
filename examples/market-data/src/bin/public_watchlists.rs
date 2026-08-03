//! tastytrade's own curated watchlists.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin public_watchlists
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

    let lists = tasty.public_watchlists(false).await?;
    info!("{} curated list(s)", lists.len());
    for list in lists.iter().take(10) {
        info!(
            "  {} [{}]: {} entr(y/ies)",
            list.name,
            list.group_name.as_deref().unwrap_or("-"),
            list.watchlist_entries.len()
        );
    }

    // The counts-only form. The venue publishes no schema for it, so what comes
    // back decodes into the same type with whatever fields arrived.
    let counts = tasty.public_watchlist_counts().await?;
    info!("{} list(s) in counts-only form", counts.len());

    Ok(())
}
