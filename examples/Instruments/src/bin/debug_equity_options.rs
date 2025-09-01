/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{info, debug, error};

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
    
    // Test the problematic endpoint
    info!("\n🔍 Testing list_all_equity_options...");
    match tasty.list_all_equity_options(0, Some(true)).await {
        Ok(paginated_options) => {
            info!("✅ Success! Retrieved {} equity options", paginated_options.items.len());
        }
        Err(e) => {
            error!("❌ Error: {}", e);
        }
    }
    
    Ok(())
}