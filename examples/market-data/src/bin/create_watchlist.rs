//! Create a throwaway watchlist, then delete it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin create_watchlist
//! ```
//!
//! **Creates user data**, so it refuses to run anywhere but certification, uses
//! a uniquely named throwaway list, and removes it again. Running an example
//! must not be able to collide with a real watchlist.

use std::time::{SystemTime, UNIX_EPOCH};

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
    if config.environment() != Environment::Certification {
        info!("This example creates a watchlist and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    // Unique, so it cannot collide with a real list even by accident.
    let name = format!(
        "tastytrade-rs-example-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    );

    let created = tasty
        .create_watchlist(
            &NewWatchlist::new(&name, &["AAPL", "SPY"])
                .with_entry("/ES", Some("Future".to_string()))
                .with_group_name("examples"),
        )
        .await?;
    info!(
        "Created {} with {} entr(y/ies)",
        created.name,
        created.watchlist_entries.len()
    );

    // A blank name never reaches the network: the name is also the URL segment
    // a later replace or delete addresses, so a list nobody can name is a list
    // nobody can remove.
    match tasty
        .create_watchlist(&NewWatchlist::new("   ", &["AAPL"]))
        .await
    {
        Ok(_) => info!("a blank name was accepted, which is a bug"),
        Err(error) => info!(
            "blank name refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    tasty.delete_watchlist(&name).await?;
    info!("Deleted {name}");

    Ok(())
}
