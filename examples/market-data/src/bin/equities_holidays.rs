//! The equities holiday calendar.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin equities_holidays
//! ```
//!
//! The reason this endpoint exists: a hardcoded exchange calendar is wrong
//! roughly once a quarter.

use chrono::Utc;
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

    let calendar = tasty.equities_holidays().await?;

    info!("{} holiday(s)", calendar.market_holidays.len());
    for holiday in calendar.market_holidays.iter().take(20) {
        info!("  closed: {holiday}");
    }
    info!("{} half day(s)", calendar.market_half_days.len());
    for half in calendar.market_half_days.iter().take(20) {
        info!("  early close: {half}");
    }

    let today = Utc::now().date_naive();
    info!(
        "Today {today}: holiday {}, half day {}",
        calendar.is_holiday(today),
        calendar.is_half_day(today)
    );

    Ok(())
}
