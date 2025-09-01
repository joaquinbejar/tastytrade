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
    
    info!("🔍 Debug equity option fields with minimal request");
    
    let config = TastyTradeConfig::from_env();
    
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }
    
    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");
    
    // Try with minimal parameters to trigger deserialization errors
    info!("\n🔍 Testing with per-page=1 to get minimal response...");
    
    // This should trigger detailed deserialization error logging
    match tasty.list_all_equity_options(0, Some(true)).await {
        Ok(paginated_options) => {
            info!("✅ Success! Found {} equity options", paginated_options.items.len());
        }
        Err(e) => {
            error!("❌ Error: {}", e);
            
            // Try with inactive options to see if that works
            info!("\n🔍 Trying with inactive options...");
            match tasty.list_all_equity_options(0, Some(false)).await {
                Ok(paginated_options) => {
                    info!("✅ Inactive options work! Found {} equity options", paginated_options.items.len());
                }
                Err(e2) => {
                    error!("❌ Inactive options also fail: {}", e2);
                }
            }
            
            // Try without active filter
            info!("\n🔍 Trying without active filter...");
            match tasty.list_all_equity_options(0, None).await {
                Ok(paginated_options) => {
                    info!("✅ No filter works! Found {} equity options", paginated_options.items.len());
                }
                Err(e3) => {
                    error!("❌ No filter also fails: {}", e3);
                }
            }
        }
    }
    
    Ok(())
}