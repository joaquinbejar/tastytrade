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

    info!("🔍 Debug equity option fields with minimal request");

    let config = TastyTradeConfig::from_env();

    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }

    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::connect(&config).await?;
    info!("✅ Successfully logged in!");

    // Test equity option deserialization using working endpoints
    info!("\n🔍 Testing equity option deserialization with functional endpoints...");

    // Test 1: Individual equity option lookup
    info!("\n📊 Test 1: Individual equity option lookup...");
    let test_symbols = vec![
        "AAPL  241220C00200000", // AAPL call option
        "SPY   241220P00500000", // SPY put option
    ];

    for symbol in &test_symbols {
        match tasty.get_equity_option(symbol, None).await {
            Ok(option) => {
                info!("✅ Successfully deserialized equity option: {}", symbol);
                info!(
                    "   📈 Details: {} ${} {} (Active: {}, Days to exp: {})",
                    option.underlying_symbol.0,
                    option.strike_price,
                    option.option_type,
                    option.active,
                    option.days_to_expiration
                );
            }
            Err(e) => {
                error!("❌ Error deserializing {}: {}", symbol, e);
            }
        }
    }

    // Test 2: Option chain deserialization (multiple options at once)
    info!("\n📊 Test 2: Option chain deserialization for AAPL...");
    match tasty.list_option_chains("AAPL").await {
        Ok(options) => {
            info!(
                "✅ Successfully deserialized {} AAPL options",
                options.len()
            );

            if !options.is_empty() {
                // Analyze the first few options for field validation
                let sample_size = std::cmp::min(5, options.len());
                info!(
                    "   🔍 Analyzing first {} options for field completeness:",
                    sample_size
                );

                for (i, option) in options.iter().take(sample_size).enumerate() {
                    info!(
                        "      {}. {} - Strike: ${}, Type: {}, Active: {}, Days: {}",
                        i + 1,
                        option.symbol.0,
                        option.strike_price,
                        option.option_type,
                        option.active,
                        option.days_to_expiration
                    );
                }

                // Statistics
                let active_count = options.iter().filter(|o| o.active).count();
                let calls = options.iter().filter(|o| o.option_type == "C").count();
                let puts = options.iter().filter(|o| o.option_type == "P").count();

                info!(
                    "   📊 Statistics: {} active, {} calls, {} puts",
                    active_count, calls, puts
                );
            }
        }
        Err(e) => {
            error!("❌ Error getting AAPL option chain: {}", e);
        }
    }

    // Test 3: Specific symbols with list_equity_options
    info!("\n📊 Test 3: Multiple specific symbols with list_equity_options...");
    match tasty.list_equity_options(&test_symbols, Some(true)).await {
        Ok(options) => {
            info!(
                "✅ Successfully deserialized {} specific equity options",
                options.len()
            );
            for option in &options {
                info!(
                    "   - {}: {} ${} {} (Exp: {})",
                    option.symbol.0,
                    option.underlying_symbol.0,
                    option.strike_price,
                    option.option_type,
                    option.expiration_date
                );
            }
        }
        Err(e) => {
            error!("❌ Error getting specific equity options: {}", e);
        }
    }

    info!("\n✅ Equity option field debugging completed!");
    info!("💡 All tests use functional endpoints that properly deserialize equity option data.");

    Ok(())
}
