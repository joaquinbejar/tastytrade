//! One snapshot of prices for several instrument types at once.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin market_data_by_type
//! ```
//!
//! The venue's sandbox page describes real-time market data as live-only, so
//! this probes certification first and reports what it finds rather than
//! assuming either way. It is read-only either way and never opts into
//! production on its own — set `TASTYTRADE_USE_DEMO=false` yourself if you want
//! that, and it will say so before reading.

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
    info!("Environment: {} (read only)", config.environment());

    let tasty = TastyTrade::connect(&config).await?;

    // Two instrument types in one call, which is the case the per-type
    // comma-separated encoding gets wrong: one key per type, sent once.
    let request = MarketDataRequest::new()
        .with_equities(&["AAPL", "TSLA"])
        .with_cryptocurrencies(&["BTC/USD"]);

    match tasty.market_data_by_type(&request).await {
        Ok(snapshots) => {
            info!("{} snapshot(s)", snapshots.len());
            for snapshot in &snapshots {
                info!(
                    "  {} {:?}: bid {} ask {} mark {} (updated {})",
                    snapshot.symbol,
                    snapshot.instrument_type,
                    show(snapshot.bid.as_ref()),
                    show(snapshot.ask.as_ref()),
                    show(snapshot.mark.as_ref()),
                    show(snapshot.updated_at.as_ref())
                );
                if snapshot.is_trading_halted == Some(true) {
                    info!("    trading is halted");
                }
            }
            if snapshots.is_empty() {
                // Worth recording: the endpoint answered, and answered with
                // nothing, which is a different fact from it not existing here.
                info!(
                    "The endpoint answered with no snapshots in {}.",
                    config.environment()
                );
            }
        }
        Err(error) => info!(
            "Market data is not available in {}: {error}",
            config.environment()
        ),
    }

    // Over the limit, which never reaches the network. Shown because a local
    // refusal and a venue rejection look nothing alike to a caller.
    let too_many: Vec<String> = (0..101).map(|i| format!("SYM{i}")).collect();
    match tasty
        .market_data_by_type(&MarketDataRequest::new().with_equities(&too_many))
        .await
    {
        Ok(_) => info!("101 symbols were accepted, which is a bug"),
        Err(error) => info!(
            "101 symbols refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
