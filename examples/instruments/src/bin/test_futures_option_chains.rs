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
    let popular_products = vec!["ES", "NQ", "ZN", "RTY", "CL", "GC"];

    for product_code in popular_products {
        info!(
            "\n📊 Testing futures option chains for product: {}",
            product_code
        );

        // Test 1: Get futures option chain (direct)
        match tasty.list_futures_option_chains(product_code).await {
            Ok(options) => {
                if !options.is_empty() {
                    // Group by underlying symbol and expiration
                    let mut symbol_stats = std::collections::HashMap::new();
                    let mut expiration_stats = std::collections::HashMap::new();
                    
                    for option in &options {
                        *symbol_stats.entry(option.underlying_symbol.0.clone()).or_insert(0) += 1;
                        *expiration_stats.entry(option.expiration_date.clone()).or_insert(0) += 1;
                    }
                    
                    let calls = options.iter().filter(|o| o.option_type == "C").count();
                    let puts = options.iter().filter(|o| o.option_type == "P").count();
                    let active = options.iter().filter(|o| o.active).count();
                    
                    info!("✅ Product {}: {} total options ({} calls, {} puts, {} active)", 
                         product_code, options.len(), calls, puts, active);
                    info!("   📊 {} underlying symbols, {} expiration dates", 
                         symbol_stats.len(), expiration_stats.len());
                    
                    // Show top symbols by option count
                    let mut sorted_symbols: Vec<_> = symbol_stats.iter().collect();
                    sorted_symbols.sort_by(|a, b| b.1.cmp(a.1));
                    
                    info!("   🎯 Top symbols by option count:");
                    for (i, (symbol, count)) in sorted_symbols.iter().take(3).enumerate() {
                        info!("      {}. {}: {} options", i + 1, symbol, count);
                    }
                } else {
                    info!("❌ Product {}: No options found", product_code);
                }
            }
            Err(e) => {
                error!("❌ Product {}: Error - {}", product_code, e);
            }
        }

        // Test 2: Nested format (temporarily enabled to debug 502 errors)
        match tasty.list_nested_futures_option_chains(product_code).await {
            Ok(nested_chains) => {
                if !nested_chains.is_empty() {
                    let mut total_strikes = 0;
                    let mut total_expirations = 0;
                    
                    for chain in &nested_chains {
                        for option_chain in &chain.option_chains {
                            total_expirations += option_chain.expirations.len();
                            for expiration in &option_chain.expirations {
                                total_strikes += expiration.strikes.len();
                            }
                        }
                    }
                    
                    let estimated_options = total_strikes * 2; // Each strike has call and put
                    
                    info!("   🔗 Nested format: {} nested chains, {} total expirations, {} strikes (~{} options)",
                         nested_chains.len(), total_expirations, total_strikes, estimated_options);
                    
                    // Show chain details
                    for (i, chain) in nested_chains.iter().take(2).enumerate() {
                        let total_chain_strikes: usize = chain.option_chains.iter()
                            .map(|oc| oc.expirations.iter().map(|e| e.strikes.len()).sum::<usize>())
                            .sum();
                        let total_chain_expirations: usize = chain.option_chains.iter()
                            .map(|oc| oc.expirations.len())
                            .sum();
                        let futures_info = if !chain.futures.is_empty() { &chain.futures[0].symbol } else { "N/A" };
                        info!("      Nested Chain {}: {} expirations, {} strikes (Future: {})", 
                             i + 1, total_chain_expirations, total_chain_strikes, futures_info);
                    }
                } else {
                    info!("   🔗 Nested format: No chains found");
                }
            }
            Err(e) => {
                error!("   🔗 Nested format: Full error details - {}", e);
            }
        }
    }

    info!("\n✅ Futures option chains testing completed!");

    Ok(())
}
