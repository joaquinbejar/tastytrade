//! The current session for one futures collection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin futures_session_current_for
//! ```

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

    for collection in [InstrumentCollection::Cme, InstrumentCollection::Cfe] {
        match tasty.current_futures_session(&collection).await {
            Ok(session) => info!(
                "{}: {} — open now {:?}",
                collection.as_wire(),
                session.state.as_deref().unwrap_or("unstated"),
                session.is_open_at(Utc::now().fixed_offset())
            ),
            Err(error) => info!("{}: {error}", collection.as_wire()),
        }
    }

    Ok(())
}
