//! The previous equities session.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin equities_session_previous
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

    let previous = tasty.previous_equities_session(None).await?;
    info!("Previous session: {:?}", previous.session_date);

    let before = tasty
        .previous_equities_session(Some((Utc::now() - Duration::days(7)).date_naive()))
        .await?;
    info!("Session before a week ago: {:?}", before.session_date);

    Ok(())
}
