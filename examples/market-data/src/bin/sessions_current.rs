//! The current session across several instrument collections.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin sessions_current
//! ```
//!
//! `instrument-collections[]` is required, so the first collection is a
//! separate argument from the rest: an empty selection cannot be built.

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

    let session = tasty
        .current_market_session(
            SessionCollection::Equity,
            &[SessionCollection::Cme, SessionCollection::Cfe],
        )
        .await?;

    info!("State: {}", session.state.as_deref().unwrap_or("unstated"));
    // Derived from the session the venue sent, never from a local guess about
    // the exchange timezone — that is what this endpoint is for.
    let now = Utc::now().fixed_offset();
    match session.is_open_at(now) {
        Some(true) => info!("Regular trading is open"),
        Some(false) => info!("Regular trading is closed"),
        None => info!("The venue did not send both boundaries, so this is unknown"),
    }
    info!("Extended: {:?}", session.is_extended_open_at(now));

    if let Some(next) = &session.next_session {
        info!("Next session opens {}", show(next.open_at.as_ref()));
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
