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
    
    info!("🚀 Testing cryptocurrencies endpoints");
    info!("======================================");
    
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
    
    // Test 1: List specific cryptocurrencies
    info!("\n📊 Test 1: Listing specific cryptocurrencies...");
    // Using official TastyTrade cryptocurrency symbols
    let crypto_symbols = vec!["BTC/USD", "ETH/USD", "LTC/USD", "BCH/USD", "DOGE/USD", "ADA/USD", "SOL/USD"];
    
    match tasty.list_cryptocurrencies(&crypto_symbols).await {
        Ok(cryptos) => {
            info!("✅ Found {} cryptocurrencies from {} requested symbols", cryptos.len(), crypto_symbols.len());
            
            if !cryptos.is_empty() {
                // Show first few cryptocurrencies
                for (i, crypto) in cryptos.iter().enumerate().take(5) {
                    debug!("   {}. {} | Description: {} | Active: {} | Closing Only: {}", 
                        i + 1, 
                        crypto.symbol.0, 
                        crypto.description,
                        crypto.active,
                        crypto.is_closing_only
                    );
                    
                    if i < 2 {
                        debug!("      - ID: {}", crypto.id);
                        debug!("      - Instrument Type: {:?}", crypto.instrument_type);
                        debug!("      - Tick Size: {}", crypto.tick_size);
                        debug!("      - Streamer Symbol: {}", crypto.streamer_symbol.0);
                        
                        if !crypto.destination_venue_symbols.is_empty() {
                            debug!("      - Destination Venues: {}", crypto.destination_venue_symbols.len());
                            for venue in &crypto.destination_venue_symbols {
                                debug!("        * {}: {} (routable: {})", 
                                    venue.destination_venue, venue.symbol.0, venue.routable);
                            }
                        }
                    }
                }
                
                if cryptos.len() > 5 {
                    debug!("   ... and {} more cryptocurrencies", cryptos.len() - 5);
                }
                
                // Analyze by status
                let active = cryptos.iter().filter(|c| c.active).count();
                let closing_only = cryptos.iter().filter(|c| c.is_closing_only).count();
                
                info!("   📈 Analysis:");
                debug!("      - Active: {}", active);
                debug!("      - Closing Only: {}", closing_only);
                debug!("      - Inactive: {}", cryptos.len() - active);
                
                // Test 2: Get individual cryptocurrencies
                info!("\n📊 Test 2: Getting individual cryptocurrencies...");
                
                // Test with first few cryptocurrencies from the list
                for crypto in cryptos.iter().take(3) {
                    match tasty.get_cryptocurrency(&crypto.symbol.0).await {
                        Ok(specific_crypto) => {
                            info!("✅ Retrieved cryptocurrency: {}", specific_crypto.symbol.0);
                            info!("   📊 Details:");
                            info!("      - Description: {}", specific_crypto.description);
                            info!("      - Active: {}", specific_crypto.active);
                            info!("      - Tick Size: {}", specific_crypto.tick_size);
                            info!("      - Streamer Symbol: {}", specific_crypto.streamer_symbol.0);
                            info!("      - Venues: {} destination venues", specific_crypto.destination_venue_symbols.len());
                            
                            // Show first few venues
                            for (i, venue) in specific_crypto.destination_venue_symbols.iter().take(3).enumerate() {
                                info!("         {}. {}: {} (routable: {})", 
                                     i + 1, venue.destination_venue, venue.symbol.0, venue.routable);
                            }
                        }
                        Err(e) => {
                            error!("❌ Error getting cryptocurrency {}: {}", crypto.symbol.0, e);
                        }
                    }
                }
                
                // Analyze destination venues
                info!("\n📊 Test 3: Analyzing destination venues...");
                let mut venue_count = std::collections::HashMap::new();
                let mut total_venues = 0;
                
                for crypto in &cryptos {
                    for venue in &crypto.destination_venue_symbols {
                        *venue_count.entry(venue.destination_venue.clone()).or_insert(0) += 1;
                        total_venues += 1;
                    }
                }
                
                info!("✅ Found {} total destination venues across {} unique venues", 
                    total_venues, venue_count.len());
                
                debug!("   📊 Venues by popularity:");
                let mut sorted_venues: Vec<_> = venue_count.iter().collect();
                sorted_venues.sort_by(|a, b| b.1.cmp(a.1));
                
                for (i, (venue, count)) in sorted_venues.iter().enumerate().take(5) {
                    debug!("      {}. {}: {} cryptocurrencies", i + 1, venue, count);
                }
                
            } else {
                info!("   ℹ️ No cryptocurrencies found for the specified symbols");
                info!("   📝 Note: The cryptocurrency endpoint requires specific symbols.");
                info!("   📝 Tried symbols: {:?}", crypto_symbols);
            }
        }
        Err(e) => {
            error!("❌ Error listing cryptocurrencies: {}", e);
            info!("   📝 Note: Make sure the symbols are in the correct format (e.g., BTC/USD)");
        }
    }
    
    info!("\n✅ Cryptocurrencies testing completed!");
    
    Ok(())
}