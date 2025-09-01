/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    
    info!("🔍 Debug equity options deserialization");
    
    let config = TastyTradeConfig::from_env();
    
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }
    
    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");
    
    // Test individual equity option lookups (working endpoint)
    // Note: These are example symbols that may or may not exist
    // The important thing is testing the deserialization, not finding specific options
    let test_symbols = vec![
        "AAPL  241220C00200000", // AAPL call option (Dec 2024)
        "SPY   241220P00500000", // SPY put option (Dec 2024)
        "MSFT  241220C00400000", // MSFT call option (Dec 2024)
    ];
    
    info!("\n🔍 Testing individual equity option lookups...");
    
    for symbol in test_symbols {
        info!("\n📊 Testing symbol: {}", symbol);
        
        match tasty.get_equity_option(symbol).await {
            Ok(option) => {
                info!("✅ Successfully retrieved equity option: {}", symbol);
                info!("   📈 Details:");
                info!("      - Underlying: {}", option.underlying_symbol.0);
                info!("      - Strike: ${}", option.strike_price);
                info!("      - Type: {}", option.option_type);
                info!("      - Expiration: {}", option.expiration_date);
                info!("      - Active: {}", option.active);
                info!("      - Days to Exp: {}", option.days_to_expiration);
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                
                if error_msg.contains("502 Bad Gateway") {
                    error!("❌ Server Error: 502 Bad Gateway for symbol {}", symbol);
                    error!("   This is a server-side issue, not a problem with the client code.");
                } else if error_msg.contains("404") || error_msg.contains("not found") {
                    error!("❌ Symbol not found: {} (this is expected for test symbols)", symbol);
                } else {
                    error!("❌ Unexpected error for {}: {}", symbol, e);
                }
            }
        }
    }
    
    // Note about the removed deprecated endpoint
    info!("\n📝 Note: The deprecated list_all_equity_options method has been removed.");
    info!("   The /instruments/equity-options endpoint was deprecated and non-functional.");
    info!("   This example now demonstrates the correct alternatives:");
    info!("   - get_equity_option(symbol) for individual options");
    info!("   - list_option_chains(underlying) for all options of an underlying");
    info!("   - list_equity_options(symbols, active) for specific symbols");
    
    // Demonstrate the correct way to get multiple equity options
    info!("\n🔍 Demonstrating correct alternatives...");
    
    // Alternative 1: Get all options for a specific underlying
    info!("\n📊 Alternative 1: Get all AAPL options using list_option_chains...");
    match tasty.list_option_chains("AAPL").await {
        Ok(options) => {
            info!("✅ Retrieved {} AAPL options using list_option_chains", options.len());
            if !options.is_empty() {
                let calls = options.iter().filter(|o| o.option_type == "C").count();
                let puts = options.iter().filter(|o| o.option_type == "P").count();
                info!("   📈 Found {} calls and {} puts", calls, puts);
            }
        }
        Err(e) => {
            error!("❌ Error getting AAPL option chain: {}", e);
        }
    }
    
    // Alternative 2: Get specific options by symbols
    info!("\n📊 Alternative 2: Get specific options using list_equity_options...");
    let specific_symbols = vec!["AAPL  241220C00200000", "SPY   241220P00500000"];
    match tasty.list_equity_options(&specific_symbols, Some(true)).await {
        Ok(options) => {
            info!("✅ Retrieved {} specific options using list_equity_options", options.len());
            for option in &options {
                info!("   - {}: {} ${} {}", option.symbol.0, option.underlying_symbol.0, option.strike_price, option.option_type);
            }
        }
        Err(e) => {
            error!("❌ Error getting specific options: {}", e);
        }
    }
    
    Ok(())
}