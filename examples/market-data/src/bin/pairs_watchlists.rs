//! Every pairs watchlist.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin pairs_watchlists
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

    for list in tasty.pairs_watchlists().await? {
        info!(
            "{}: {} equation(s), order index {:?}",
            list.name,
            list.pairs_equations.len(),
            list.order_index
        );
    }

    Ok(())
}
