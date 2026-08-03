//! Replace a watchlist — every property of it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin replace_watchlist
//! ```
//!
//! **Destroys user data.** `PUT /watchlists/{name}` is not an append and not a
//! merge: the entries sent are the entries that survive. Anything on the list
//! and not in the request is gone.
//!
//! Certification only, on a uniquely named throwaway list, cleaned up
//! afterwards.

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
        info!("This example replaces a watchlist and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    let name = format!(
        "tastytrade-rs-example-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    );

    tasty
        .create_watchlist(&NewWatchlist::new(&name, &["AAPL", "SPY", "QQQ"]))
        .await?;
    info!("Created {name} with three entries");

    // The replacement carries one symbol, so the other two are gone. This is
    // the behaviour worth demonstrating: it is not an append.
    let replaced = tasty
        .replace_watchlist(&name, &NewWatchlist::new(&name, &["TSLA"]))
        .await?;
    info!(
        "After replacing: {} entr(y/ies) — {}",
        replaced.watchlist_entries.len(),
        replaced
            .watchlist_entries
            .iter()
            .map(|entry| entry.symbol.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // To *add* a symbol, read the list and send the whole thing back.
    let mut appended = tasty.watchlist(&name).await?;
    appended.watchlist_entries.push(WatchlistEntry {
        symbol: "AAPL".into(),
        instrument_type: Some("Equity".to_string()),
    });
    let grown = tasty
        .replace_watchlist(
            &name,
            &NewWatchlist {
                name: appended.name.clone(),
                watchlist_entries: appended.watchlist_entries,
                group_name: appended.group_name,
                order_index: appended.order_index,
            },
        )
        .await?;
    info!(
        "After appending: {} entr(y/ies)",
        grown.watchlist_entries.len()
    );

    tasty.delete_watchlist(&name).await?;
    info!("Deleted {name}");

    Ok(())
}
