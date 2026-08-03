//! One pairs watchlist by name.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin pairs_watchlist
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

    let Some(first) = tasty.pairs_watchlists().await?.into_iter().next() else {
        info!("The venue published no pairs watchlists.");
        return Ok(());
    };

    let list = tasty.pairs_watchlist(&first.name).await?;
    info!("{}: {} equation(s)", list.name, list.pairs_equations.len());
    // The equations have no published schema, so they are printed as the JSON
    // that arrived rather than through a type invented for them.
    for equation in list.pairs_equations.iter().take(10) {
        info!("  {equation}");
    }

    Ok(())
}
