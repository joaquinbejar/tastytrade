//! Simulate one trade.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin simulate_trade
//! ```
//!
//! **Simulates.** Nothing routes and no position changes.
//!
//! The request body is passed through as JSON because the published document
//! describes it only as an object. A guessed type would refuse requests the
//! venue accepts, which is worse than making the caller write the JSON — and it
//! becomes a modelled type as soon as a real payload is captured.

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

    let request = serde_json::json!({
        "symbol": "SPY",
        "legs": [{
            "type": "Put",
            "direction": "Short",
            "quantity": 1,
            "strikeSelection": "Delta",
            "delta": 0.16,
            "daysUntilExpiration": 45
        }]
    });

    match tasty.simulate_trade(&request).await {
        Ok(result) => info!("Simulated: {result}"),
        // What this endpoint accepts is undocumented, so a rejection is data
        // about the contract rather than a failure to hide.
        Err(error) => info!("The venue rejected the simulation: {error}"),
    }

    Ok(())
}
