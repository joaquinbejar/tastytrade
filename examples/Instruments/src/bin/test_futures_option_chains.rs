/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    info!("🚀 Testing futures option chains endpoints");
    info!("============================================");

    // Load configuration from environment
    let config = TastyTradeConfig::from_env();

    // Check if we have valid credentials
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found. Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables.");
        return Err("Missing credentials".into());
    }

    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");

    // Test popular futures product codes
    let popular_products = vec!["ES", "NQ", "YM", "RTY", "CL", "GC"];

    for product_code in popular_products {
        info!(
            "\n📊 Testing futures option chains for product: {}",
            product_code
        );

        // Test 1: Get futures option chain (direct)
        match tasty.list_futures_option_chains(product_code).await {
            Ok(options) => {
                info!(
                    "✅ Found {} future options for product {}",
                    options.len(),
                    product_code
                );

                if !options.is_empty() {
                    // Show first few options
                    for (i, option) in options.iter().enumerate().take(3) {
                        debug!(
                            "   {}. {} | Strike: ${} | Exp: {} | Type: {} | Active: {}",
                            i + 1,
                            option.symbol.0,
                            option.strike_price,
                            option.expiration_date,
                            option.option_type,
                            option.active
                        );
                    }
                    if options.len() > 3 {
                        debug!("   ... and {} more options", options.len() - 3);
                    }

                    // Analyze option types
                    let calls = options.iter().filter(|o| o.option_type == "C").count();
                    let puts = options.iter().filter(|o| o.option_type == "P").count();
                    let active = options.iter().filter(|o| o.active).count();

                    info!(
                        "   📈 Analysis: {} calls, {} puts, {} active",
                        calls, puts, active
                    );
                } else {
                    info!("   ℹ️ No options found for product {}", product_code);
                }
            }
            Err(e) => {
                error!(
                    "❌ Error getting futures option chain for {}: {}",
                    product_code, e
                );
            }
        }

        // Test 2: Compare with nested version
        match tasty.list_nested_futures_option_chains(product_code).await {
            Ok(nested_chains) => {
                info!(
                    "✅ Found {} nested option chains for product {}",
                    nested_chains.len(),
                    product_code
                );

                if !nested_chains.is_empty() {
                    let total_options: usize = nested_chains
                        .iter()
                        .map(|chain| {
                            chain
                                .expirations
                                .iter()
                                .map(|exp| exp.strikes.len() * 2) // Each strike has call and put
                                .sum::<usize>()
                        })
                        .sum();

                    info!(
                        "   📊 Nested format contains ~{} total options across {} chains",
                        total_options,
                        nested_chains.len()
                    );
                }
            }
            Err(e) => {
                error!(
                    "❌ Error getting nested futures option chains for {}: {}",
                    product_code, e
                );
            }
        }
    }

    info!("\n✅ Futures option chains testing completed!");

    Ok(())
}