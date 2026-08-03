//! Implied volatility and liquidity for several underlyings at once.
//!
//! **Live only.** The venue's sandbox page lists Market Metrics as unavailable
//! in certification, so this requires an explicit read-only production opt-in:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false TASTYTRADE_ALLOW_PRODUCTION_READ=1 \
//!   cargo run -p market-data --bin market_metrics
//! ```
//!
//! Read-only. Nothing here places, replaces or cancels anything.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

const OPT_IN: &str = "TASTYTRADE_ALLOW_PRODUCTION_READ";

/// Several symbols in one call, which is where the comma-joined `symbols`
/// parameter matters: repeated keys would return metrics for one of them.
const SYMBOLS: [&str; 3] = ["AAPL", "TSLA", "SPY"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    info!("Environment: {}", config.environment());
    if config.environment() != Environment::Production {
        info!(
            "Market Metrics is not served in certification. Re-run with \
             TASTYTRADE_USE_DEMO=false and {OPT_IN}=1 to read it from production."
        );
        return Ok(());
    }
    // The exact value, not merely a set variable. An exported `=0`, an empty
    // string left over from a shell profile, or a stale `=false` all read as
    // present, and the usage text promises `=1`.
    if std::env::var(OPT_IN)
        .map(|value| value.trim() != "1")
        .unwrap_or(true)
    {
        info!("This would read from PRODUCTION. Set {OPT_IN}=1 to allow it (read only).");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let metrics = tasty.market_metrics(&SYMBOLS).await?;

    info!("{} metric(s) — READ ONLY", metrics.len());
    for metric in &metrics {
        info!(
            "{}: IV {} rank {} percentile {} liquidity rating {}",
            metric.symbol,
            show(metric.implied_volatility_index.as_ref()),
            show(metric.implied_volatility_rank.as_ref()),
            show(metric.implied_volatility_percentile.as_ref()),
            show(metric.liquidity_rating.as_ref())
        );
        for expiration in metric.option_expiration_implied_volatilities.iter().take(5) {
            info!(
                "    {} ({}): IV {}",
                // A calendar day, whichever shape the venue sent.
                show(expiration.expiration_date.as_ref()),
                expiration.settlement_type.as_deref().unwrap_or("-"),
                show(expiration.implied_volatility.as_ref())
            );
        }
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
