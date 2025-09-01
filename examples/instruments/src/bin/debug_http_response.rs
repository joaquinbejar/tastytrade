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
    
    info!("🔍 Debug HTTP response for equity options");
    
    let config = TastyTradeConfig::from_env();
    
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }
    
    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");
    
    // Test different equity option endpoints to isolate the problem
    info!("\n🔍 Testing list_equity_options with specific symbols...");
    match tasty.list_equity_options(&["AAPL"], Some(true)).await {
        Ok(options) => {
            info!("✅ list_equity_options works! Found {} options", options.len());
        }
        Err(e) => {
            error!("❌ list_equity_options failed: {}", e);
        }
    }
    
    info!("\n🔍 Testing list_equity_options with inactive options...");
    match tasty.list_equity_options(&["AAPL"], Some(false)).await {
        Ok(options) => {
            info!("✅ list_equity_options (inactive) works! Found {} options", options.len());
        }
        Err(e) => {
            error!("❌ list_equity_options (inactive) failed: {}", e);
        }
    }
    
    info!("\n🔍 Testing list_equity_options without active filter...");
    match tasty.list_equity_options(&["AAPL"], None).await {
        Ok(options) => {
            info!("✅ list_equity_options (no filter) works! Found {} options", options.len());
        }
        Err(e) => {
            error!("❌ list_equity_options (no filter) failed: {}", e);
        }
    }
    
    info!("\n🔍 Testing list_all_equity_options (the problematic one)...");
    match tasty.list_all_equity_options(0, Some(true)).await {
        Ok(paginated_options) => {
            info!("✅ list_all_equity_options works! Found {} options", paginated_options.items.len());
        }
        Err(e) => {
            error!("❌ list_all_equity_options failed: {}", e);
        }
    }
    
    Ok(())
}