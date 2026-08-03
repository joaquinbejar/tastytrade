//! Delete a watchlist.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin delete_watchlist
//! ```
//!
//! **Destroys user data, irreversibly.** Certification only, and only on a list
//! this example's own naming convention created — so it cannot remove a real
//! one.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// Only lists created by these examples are touched.
const PREFIX: &str = "tastytrade-rs-example-";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    if config.environment() != Environment::Certification {
        info!("This example deletes a watchlist and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    // The guard that matters: a delete is irreversible, so this only ever
    // touches a list whose name says it came from here.
    let disposable: Vec<_> = tasty
        .watchlists()
        .await?
        .into_iter()
        .filter(|list| list.name.starts_with(PREFIX))
        .collect();

    if disposable.is_empty() {
        info!("No list named {PREFIX}* to delete. Run create_watchlist first.");
        return Ok(());
    }

    for list in disposable {
        tasty.delete_watchlist(&list.name).await?;
        info!("Deleted {}", list.name);
    }

    Ok(())
}
