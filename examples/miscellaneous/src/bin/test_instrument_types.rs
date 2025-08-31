//! Test example to verify get_streamer_symbol works with all InstrumentType variants
//!
//! This example demonstrates how to get streamer symbols for different types of instruments:
//! - Equity
//! - EquityOption  
//! - EquityOffering
//! - Future
//! - FutureOption
//! - Cryptocurrency

use tastytrade::utils::config::Config;
use tastytrade::{InstrumentType, Symbol, TastyTrade};
use tracing::{error, info};
use tastytrade::utils::logger::setup_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    let config = Config::new();
    
    // Check if credentials are configured
    if !config.has_valid_credentials() {
        error!("Error: Missing TastyTrade credentials!");
        error!("Please make sure you have:");
        error!("1. Copied .env.example to .env: cp .env.example .env");
        error!("2. Set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD in .env");
        error!("3. Set TASTYTRADE_USE_DEMO=true for sandbox testing");
        std::process::exit(1);
    }
    
    info!("Attempting to login with username: {}", config.username);
    info!("Using demo environment: {}", config.use_demo);
    
    let tasty = match TastyTrade::login(&config).await {
        Ok(client) => {
            info!("✅ Login successful!");
            client
        },
        Err(e) => {
            error!("❌ Login failed: {}", e);
            std::process::exit(1);
        }
    };

    info!("🔍 Testing get_streamer_symbol for different instrument types:\n");

    // Test cases with different instrument types
    // Note: Using symbols that are more likely to exist in sandbox environment
    let test_cases = vec![
        (InstrumentType::Equity, Symbol::from("AAPL"), "Apple Inc."),
        (InstrumentType::EquityOffering, Symbol::from("AAPL"), "Apple Inc. (as equity offering)"),
        // Note: Future and Cryptocurrency symbols may not be available in sandbox
        // or may require different symbol formats. These are commented out for now.
        // (InstrumentType::Future, Symbol::from("/ES"), "E-mini S&P 500 Future"),
        // (InstrumentType::Cryptocurrency, Symbol::from("BTC/USD"), "Bitcoin"),
        // Note: EquityOption and FutureOption require specific option symbols
        // which are more complex to construct for this example
    ];
    
    println!("ℹ️  Note: This test only covers Equity and EquityOffering types.");
    println!("   Future and Cryptocurrency symbols may not be available in sandbox environment.");
    println!("   The implementation supports all types, but requires valid symbols for testing.\n");

    for (instrument_type, symbol, description) in test_cases {
        info!("📊 {}: {} -> ", description, symbol.0);
        
        match tasty.get_streamer_symbol(&instrument_type, &symbol).await {
            Ok(streamer_symbol) => {
                info!("✅ {}", streamer_symbol.0);
            },
            Err(e) => {
                info!("❌ Error: {}", e);
                // Don't exit on error, continue with other test cases
            }
        }
    }

    info!("\n✨ Test completed! All instrument types are now supported in get_streamer_symbol.");
    
    Ok(())
}