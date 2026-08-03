//! Search every instrument type at once, with classification filters.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin search_instruments
//! ```
//!
//! Shows the two things easy to get wrong: the classification filters are
//! **comma-joined into one parameter each** — the opposite of the instrument
//! listings' repeated keys — and `limit` is capped, with the refusal happening
//! locally before anything is sent.

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

    // Several types in one call, which is where the comma-joined encoding
    // matters: repeated keys would search for one of them.
    let filter = InstrumentSearchFilter::for_query("gold")
        .with_types(&["Equity", "Future"])
        .with_limit(10);

    let results = tasty.search_instruments(&filter).await?;
    info!("{} instrument(s) matched", results.len());

    for result in &results {
        info!(
            "  {} — {} [{} / {}]",
            result.symbol,
            result.description.as_deref().unwrap_or("no description"),
            result.instrument_type.as_deref().unwrap_or("unknown type"),
            result.category.as_deref().unwrap_or("uncategorised")
        );
        if let Some(stops) = result.stops_trading_at {
            // The offset the venue sent, preserved rather than normalised.
            info!("      stops trading at {stops}");
        }
    }

    // An equity-only search restricted to ETFs, which is the documented use of
    // `instrument-sub-type`.
    let etfs = tasty
        .search_instruments(
            &InstrumentSearchFilter::for_query("index")
                .with_types(&["Equity"])
                .with_instrument_sub_types(&["ETF"])
                .with_limit(5),
        )
        .await?;
    info!("{} ETF(s) matched", etfs.len());

    // Over the cap, so this never reaches the network. Shown because a local
    // refusal and a venue rejection look nothing alike to a caller: this one
    // is not retryable, and says so.
    let over_limit =
        InstrumentSearchFilter::for_query("anything").with_limit(MAX_SEARCH_RESULTS + 1);
    match tasty.search_instruments(&over_limit).await {
        Ok(_) => info!("the cap was not enforced, which is a bug"),
        Err(error) => info!(
            "refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    Ok(())
}
