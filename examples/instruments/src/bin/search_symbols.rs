//! Prefix search over symbols and company names.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin search_symbols
//! ```
//!
//! One of the queries below contains a `/`, which is the case that mattered:
//! the search term is a **path segment**, so an unencoded separator does not
//! fail the request — it selects a different route and the venue answers it.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// The last two are the point: a class separator and a futures symbol, both of
/// which carry characters that are not path-safe.
const QUERIES: [&str; 4] = ["AAPL", "tesla", "BRK/B", "/ES"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    for query in QUERIES {
        let results = tasty.search_symbols(query).await?;
        info!("{query:?} matched {} symbol(s)", results.len());

        for result in results.iter().take(5) {
            info!(
                "  {} — {} ({}, options: {})",
                result.symbol,
                result.description.as_deref().unwrap_or("no description"),
                result.instrument_type.as_deref().unwrap_or("unknown type"),
                // `None` is "the venue did not say", which is not "no".
                match result.options {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unstated",
                }
            );
        }
    }

    Ok(())
}
