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
    
    info!("🔍 Testing equity options with smaller page size");
    
    let config = TastyTradeConfig::from_env();
    
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found.");
        return Err("Missing credentials".into());
    }
    
    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    info!("✅ Successfully logged in!");
    
    // Try with a very small page size to see if we can get some data
    info!("\n🔍 Testing with page size 1...");
    match tasty.list_all_equity_options(0, Some(true)).await {
        Ok(paginated_options) => {
            info!("✅ Success! Found {} equity options", paginated_options.items.len());
            info!("📊 Pagination: page {}, {} items per page, {} total items", 
                paginated_options.pagination.page_offset,
                paginated_options.pagination.per_page,
                paginated_options.pagination.total_items
            );
        }
        Err(e) => {
            error!("❌ Error: {}", e);
        }
    }
    
    Ok(())
}