//! The holiday calendar for one futures collection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin futures_holidays
//! ```

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

    for collection in [FuturesExchange::Cme, FuturesExchange::Cfe] {
        match tasty.futures_holidays(collection).await {
            Ok(calendars) => info!(
                "{}: {} calendar(s), {} holiday(s), {} half day(s)",
                collection.as_wire(),
                calendars.len(),
                calendars
                    .iter()
                    .map(|calendar| calendar.market_holidays.len())
                    .sum::<usize>(),
                calendars
                    .iter()
                    .map(|calendar| calendar.market_half_days.len())
                    .sum::<usize>()
            ),
            Err(error) => info!("{}: {error}", collection.as_wire()),
        }
    }

    Ok(())
}
