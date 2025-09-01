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
    
    info!("🚀 Testing warrants endpoints");
    info!("==============================");
    
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
    
    // Test 1: List all warrants
    info!("\n📊 Test 1: Listing all warrants...");
    
    match tasty.list_warrants(None::<&[&str]>).await {
        Ok(warrants) => {
            info!("✅ Found {} warrants total", warrants.len());
            
            if !warrants.is_empty() {
                // Show first few warrants
                for (i, warrant) in warrants.iter().enumerate().take(5) {
                    debug!("   {}. {} | Description: {} | Market: {} | Active: {} | Closing Only: {}", 
                        i + 1, 
                        warrant.symbol.0, 
                        warrant.description,
                        warrant.listed_market,
                        warrant.active,
                        warrant.is_closing_only
                    );
                    
                    if i < 2 {
                        debug!("      - Instrument Type: {:?}", warrant.instrument_type);
                    }
                }
                
                if warrants.len() > 5 {
                    debug!("   ... and {} more warrants", warrants.len() - 5);
                }
                
                // Analyze warrants by various criteria
                let active = warrants.iter().filter(|w| w.active).count();
                let closing_only = warrants.iter().filter(|w| w.is_closing_only).count();
                
                info!("   📈 Analysis:");
                debug!("      - Active: {}", active);
                debug!("      - Closing Only: {}", closing_only);
                debug!("      - Inactive: {}", warrants.len() - active);
                
                // Group by listed market
                let mut market_count = std::collections::HashMap::new();
                for warrant in &warrants {
                    *market_count.entry(warrant.listed_market.clone()).or_insert(0) += 1;
                }
                
                debug!("   📊 Warrants by market:");
                let mut sorted_markets: Vec<_> = market_count.iter().collect();
                sorted_markets.sort_by(|a, b| b.1.cmp(a.1));
                
                for (i, (market, count)) in sorted_markets.iter().enumerate().take(5) {
                    debug!("      {}. {}: {} warrants", i + 1, market, count);
                }
                
                // Group by instrument type
                let mut type_count = std::collections::HashMap::new();
                for warrant in &warrants {
                    *type_count.entry(format!("{:?}", warrant.instrument_type)).or_insert(0) += 1;
                }
                
                debug!("   📊 Warrants by instrument type:");
                for (inst_type, count) in type_count.iter() {
                    debug!("      - {}: {} warrants", inst_type, count);
                }
                
                // Test 2: Get specific warrants by symbols
                info!("\n📊 Test 2: Getting specific warrants by symbols...");
                
                // Test with first few warrant symbols
                let test_symbols: Vec<_> = warrants.iter().take(3).map(|w| w.symbol.0.as_str()).collect();
                
                if !test_symbols.is_empty() {
                    match tasty.list_warrants(Some(&test_symbols)).await {
                        Ok(specific_warrants) => {
                            info!("✅ Retrieved {} specific warrants by symbols", specific_warrants.len());
                            
                            for warrant in &specific_warrants {
                                debug!("   📊 {}: {}", warrant.symbol.0, warrant.description);
                                debug!("      - Market: {} | Active: {} | Closing Only: {}", 
                                    warrant.listed_market, warrant.active, warrant.is_closing_only);
                            }
                        }
                        Err(e) => {
                            error!("❌ Error getting specific warrants by symbols: {}", e);
                        }
                    }
                    
                    // Test 3: Get individual warrant details
                    info!("\n📊 Test 3: Getting individual warrant details...");
                    
                    for symbol in test_symbols.iter().take(2) {
                        match tasty.get_warrant(symbol).await {
                            Ok(warrant) => {
                                info!("✅ Retrieved warrant details for {}", warrant.symbol.0);
                                debug!("   📊 Full details:");
                                debug!("      - Symbol: {}", warrant.symbol.0);
                                debug!("      - Instrument Type: {:?}", warrant.instrument_type);
                                debug!("      - Listed Market: {}", warrant.listed_market);
                                debug!("      - Description: {}", warrant.description);
                                debug!("      - Is Closing Only: {}", warrant.is_closing_only);
                                debug!("      - Active: {}", warrant.active);
                            }
                            Err(e) => {
                                error!("❌ Error getting warrant details for {}: {}", symbol, e);
                            }
                        }
                    }
                } else {
                    info!("   ℹ️ No warrant symbols available for individual testing");
                }
                
                // Test 4: Analyze warrant descriptions for patterns
                info!("\n📊 Test 4: Analyzing warrant descriptions...");
                
                let mut description_keywords = std::collections::HashMap::new();
                
                for warrant in &warrants {
                    // Extract common keywords from descriptions
                    let description_lower = warrant.description.to_lowercase();
                    
                    if description_lower.contains("warrant") {
                        *description_keywords.entry("warrant".to_string()).or_insert(0) += 1;
                    }
                    if description_lower.contains("call") {
                        *description_keywords.entry("call".to_string()).or_insert(0) += 1;
                    }
                    if description_lower.contains("put") {
                        *description_keywords.entry("put".to_string()).or_insert(0) += 1;
                    }
                    if description_lower.contains("right") {
                        *description_keywords.entry("right".to_string()).or_insert(0) += 1;
                    }
                    if description_lower.contains("purchase") {
                        *description_keywords.entry("purchase".to_string()).or_insert(0) += 1;
                    }
                    if description_lower.contains("common") {
                        *description_keywords.entry("common".to_string()).or_insert(0) += 1;
                    }
                }
                
                info!("✅ Analyzed warrant descriptions for common keywords");
                debug!("   📊 Keyword frequency:");
                
                let mut sorted_keywords: Vec<_> = description_keywords.iter().collect();
                sorted_keywords.sort_by(|a, b| b.1.cmp(a.1));
                
                for (keyword, count) in sorted_keywords.iter() {
                    debug!("      - '{}': {} warrants", keyword, count);
                }
                
                // Show some sample descriptions
                debug!("   📊 Sample warrant descriptions:");
                for (i, warrant) in warrants.iter().enumerate().take(3) {
                    debug!("      {}. {}: {}", i + 1, warrant.symbol.0, warrant.description);
                }
                
            } else {
                info!("   ℹ️ No warrants found");
            }
        }
        Err(e) => {
            error!("❌ Error listing warrants: {}", e);
        }
    }
    
    info!("\n✅ Warrants testing completed!");
    
    Ok(())
}