//! The previous session for one futures collection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin futures_session_previous
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

    let previous = tasty
        .previous_futures_session(&InstrumentCollection::Cme, None)
        .await?;
    info!("Previous CME session: {:?}", previous.session_date);

    let earlier = tasty
        .previous_futures_session(
            &InstrumentCollection::Cme,
            Some((Utc::now() - Duration::days(7)).date_naive()),
        )
        .await?;
    info!("CME session before a week ago: {:?}", earlier.session_date);

    Ok(())
}
