//! Session timings over a date range.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin sessions_range
//! ```
//!
//! `to-date` is required by the venue, so `SessionRange` carries it and it
//! cannot be omitted. The nine-month limit is enforced locally, before the
//! round trip.

use chrono::{Duration, Utc};
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

    let today = Utc::now().date_naive();
    let range = SessionRange::between(today, today + Duration::days(14))
        .with_instrument_collection(InstrumentCollection::Equity);

    for session in tasty.market_sessions(&range).await?.iter().take(10) {
        info!(
            "{}: open {} close {} (extended to {})",
            show(session.session_date.as_ref()),
            show(session.open_at.as_ref()),
            show(session.close_at.as_ref()),
            show(session.close_at_ext.as_ref())
        );
    }

    // Over nine months, which never reaches the network.
    let too_long = SessionRange::between(today, today + Duration::days(400));
    match tasty.market_sessions(&too_long).await {
        Ok(_) => info!("a year was accepted, which is a bug"),
        Err(error) => info!(
            "a year refused locally, retryable: {} — {error}",
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
