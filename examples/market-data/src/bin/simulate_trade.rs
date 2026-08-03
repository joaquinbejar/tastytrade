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

use rust_decimal::Decimal;
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

    // The covered call from the published request examples. This endpoint
    // takes instruments that already exist, by symbol — it is not a backtest,
    // which describes a strike to select.
    let request = SimulateTrade::new(
        "SPY",
        vec![
            SimulatedLeg {
                symbol: "SPY   190227C00275000".to_string(),
                direction: BacktestDirection::Short,
                quantity: Decimal::ONE,
            },
            SimulatedLeg {
                symbol: "SPY".to_string(),
                direction: BacktestDirection::Long,
                quantity: Decimal::from(100),
            },
        ],
    );

    match tasty.simulate_trade(&request).await {
        Ok(points) => {
            info!("Simulated over {} point(s)", points.len());
            for point in points.iter().take(10) {
                println!(
                    "  {}: {} {} (underlying {})",
                    point.date_time.as_deref().unwrap_or("-"),
                    point
                        .price
                        .map(|price| price.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    point.effect.as_deref().unwrap_or("-"),
                    point
                        .underlying_price
                        .map(|price| price.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }
        Err(error) => info!("The venue rejected the simulation: {error}"),
    }

    Ok(())
}
