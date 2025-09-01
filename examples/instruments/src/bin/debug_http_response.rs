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
            info!(
                "✅ list_equity_options works! Found {} options",
                options.len()
            );
        }
        Err(e) => {
            error!("❌ list_equity_options failed: {}", e);
        }
    }

    info!("\n🔍 Testing list_equity_options with inactive options...");
    match tasty.list_equity_options(&["AAPL"], Some(false)).await {
        Ok(options) => {
            info!(
                "✅ list_equity_options (inactive) works! Found {} options",
                options.len()
            );
        }
        Err(e) => {
            error!("❌ list_equity_options (inactive) failed: {}", e);
        }
    }

    info!("\n🔍 Testing list_equity_options without active filter...");
    match tasty.list_equity_options(&["AAPL"], None).await {
        Ok(options) => {
            info!(
                "✅ list_equity_options (no filter) works! Found {} options",
                options.len()
            );
        }
        Err(e) => {
            error!("❌ list_equity_options (no filter) failed: {}", e);
        }
    }

    info!("\n🔍 Testing list_option_chains (recommended alternative)...");
    match tasty.list_option_chains("AAPL").await {
        Ok(options) => {
            info!(
                "✅ list_option_chains works! Found {} AAPL options",
                options.len()
            );
        }
        Err(e) => {
            error!("❌ list_option_chains failed: {}", e);
        }
    }

    info!("\n🔍 Testing get_equity_option (individual lookup)...");
    match tasty.get_equity_option("AAPL  241220C00200000").await {
        Ok(option) => {
            info!(
                "✅ get_equity_option works! Retrieved option: {}",
                option.symbol.0
            );
        }
        Err(e) => {
            error!("❌ get_equity_option failed: {}", e);
        }
    }

    info!("\n📝 Note: The deprecated list_all_equity_options method has been removed.");
    info!("   It was using a deprecated API endpoint that returned 502 Bad Gateway errors.");
    info!("   The above alternatives provide the same functionality with working endpoints.");

    Ok(())
}
