/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    info!("🔍 Testing equity options with smaller page size");

    let config = TastyTradeConfig::from_env();

    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }

    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");

    // Test with functional equity option endpoints
    info!("\n🔍 Testing with individual equity option lookup...");
    let test_symbol = "AAPL  241220C00200000";
    match tasty.get_equity_option(test_symbol).await {
        Ok(option) => {
            info!("✅ Success! Retrieved equity option: {}", option.symbol.0);
            info!(
                "📊 Details: {} ${} {} (Active: {}, Days to exp: {})",
                option.underlying_symbol.0,
                option.strike_price,
                option.option_type,
                option.active,
                option.days_to_expiration
            );
        }
        Err(e) => {
            error!("❌ Error getting individual option: {}", e);
        }
    }

    info!("\n🔍 Testing with option chain for AAPL...");
    match tasty.list_option_chains("AAPL").await {
        Ok(options) => {
            info!("✅ Success! Found {} AAPL options", options.len());
            if !options.is_empty() {
                let active_count = options.iter().filter(|o| o.active).count();
                let calls = options.iter().filter(|o| o.option_type == "C").count();
                let puts = options.iter().filter(|o| o.option_type == "P").count();
                info!(
                    "📊 Analysis: {} active, {} calls, {} puts",
                    active_count, calls, puts
                );
            }
        }
        Err(e) => {
            error!("❌ Error getting option chain: {}", e);
        }
    }

    info!("\n📝 Note: The deprecated list_all_equity_options method has been removed.");
    info!("   The original endpoint was deprecated and returned server errors.");
    info!("   These functional alternatives provide better reliability and performance.");

    Ok(())
}
