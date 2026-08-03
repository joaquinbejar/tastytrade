//! The next session for one futures collection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin futures_session_next
//! ```

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

    let next = tasty
        .next_futures_session(FuturesExchange::Cme, None)
        .await?;
    info!("Next CME session: {:?}", next.session_date);

    let later = tasty
        .next_futures_session(
            FuturesExchange::Cme,
            Some((Utc::now() + Duration::days(7)).date_naive()),
        )
        .await?;
    info!("Next CME session a week out: {:?}", later.session_date);

    Ok(())
}
