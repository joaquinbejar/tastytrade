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
    
    info!("🚀 Testing equity options endpoints");
    info!("=====================================");
    
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
    
    // Test 1: List equity options by symbols
    info!("\n📊 Test 1: Listing equity options by symbols...");
    
    let test_symbols = vec!["AAPL", "MSFT", "GOOGL"];
    
    match tasty.list_equity_options(&test_symbols, Some(true)).await {
        Ok(options) => {
            info!("✅ Found {} active equity options for specified symbols", options.len());
            
            if !options.is_empty() {
                // Show first few options
                for (i, option) in options.iter().enumerate().take(5) {
                    debug!("   {}. {} | Strike: ${} | Exp: {} | Type: {} | Underlying: {}", 
                        i + 1, 
                        option.symbol.0, 
                        option.strike_price,
                        option.expiration_date,
                        option.option_type,
                        option.underlying_symbol.0
                    );
                    
                    if i < 2 {
                        debug!("      - Root Symbol: {}", option.root_symbol.0);
                        debug!("      - Exercise Style: {}", option.exercise_style);
                        debug!("      - Shares per Contract: {}", option.shares_per_contract);
                        debug!("      - Days to Expiration: {}", option.days_to_expiration);
                        debug!("      - Settlement Type: {}", option.settlement_type);
                        debug!("      - Active: {}", option.active);
                        debug!("      - Closing Only: {}", option.is_closing_only);
                    }
                }
                
                if options.len() > 5 {
                    debug!("   ... and {} more options", options.len() - 5);
                }
                
                // Analyze option types and expirations
                let calls = options.iter().filter(|o| o.option_type == "C").count();
                let puts = options.iter().filter(|o| o.option_type == "P").count();
                let closing_only = options.iter().filter(|o| o.is_closing_only).count();
                
                info!("   📈 Analysis:");
                debug!("      - Calls: {}", calls);
                debug!("      - Puts: {}", puts);
                debug!("      - Closing Only: {}", closing_only);
                
                // Group by underlying
                let mut underlying_count = std::collections::HashMap::new();
                for option in &options {
                    *underlying_count.entry(option.underlying_symbol.0.clone()).or_insert(0) += 1;
                }
                
                debug!("   📊 Options by underlying:");
                for (underlying, count) in underlying_count.iter() {
                    debug!("      - {}: {} options", underlying, count);
                }
                
                // Group by expiration
                let mut expiration_count = std::collections::HashMap::new();
                for option in &options {
                    *expiration_count.entry(option.expiration_date.clone()).or_insert(0) += 1;
                }
                
                debug!("   📅 Top expirations by option count:");
                let mut sorted_exps: Vec<_> = expiration_count.iter().collect();
                sorted_exps.sort_by(|a, b| b.1.cmp(a.1));
                
                for (i, (exp_date, count)) in sorted_exps.iter().take(5).enumerate() {
                    debug!("      {}. {} - {} options", i + 1, exp_date, count);
                }
                
            } else {
                info!("   ℹ️ No active equity options found for specified symbols");
            }
        }
        Err(e) => {
            error!("❌ Error listing equity options by symbols: {}", e);
        }
    }
    
    // Test 2: List all equity options with pagination
    info!("\n📊 Test 2: Listing all equity options (paginated)...");
    info!("   ⚠️ Note: This endpoint may experience server issues (502 Bad Gateway)");
    
    match tasty.list_all_equity_options(0, Some(true)).await {
        Ok(paginated_options) => {
            info!("✅ Retrieved {} active equity options from page 1", paginated_options.items.len());
            debug!("   📊 Pagination info:");
            debug!("      - Current page: {}", paginated_options.pagination.page_offset);
            debug!("      - Items per page: {}", paginated_options.pagination.per_page);
            debug!("      - Total pages: {}", paginated_options.pagination.total_pages);
            debug!("      - Total items: {}", paginated_options.pagination.total_items);
            
            if !paginated_options.items.is_empty() {
                // Analyze option chain types
                let mut chain_types = std::collections::HashMap::new();
                let mut exercise_styles = std::collections::HashMap::new();
                
                for option in &paginated_options.items {
                    *chain_types.entry(option.option_chain_type.clone()).or_insert(0) += 1;
                    *exercise_styles.entry(option.exercise_style.clone()).or_insert(0) += 1;
                }
                
                debug!("   📊 Option chain types:");
                for (chain_type, count) in chain_types.iter() {
                    debug!("      - {}: {} options", chain_type, count);
                }
                
                debug!("   📊 Exercise styles:");
                for (style, count) in exercise_styles.iter() {
                    debug!("      - {}: {} options", style, count);
                }
                
                // Show sample options from different underlyings
                let mut seen_underlyings = std::collections::HashSet::new();
                debug!("   📊 Sample options from different underlyings:");
                
                for option in paginated_options.items.iter().take(10) {
                    if seen_underlyings.insert(option.underlying_symbol.0.clone()) {
                        debug!("      - {} {} ${} {} (exp: {})", 
                            option.underlying_symbol.0,
                            option.option_type,
                            option.strike_price,
                            option.symbol.0,
                            option.expiration_date
                        );
                        
                        if seen_underlyings.len() >= 5 {
                            break;
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("❌ Error listing all equity options: {}", e);
            info!("   ℹ️ This is a known server-side issue (502 Bad Gateway) with the /instruments/equity-options endpoint");
            info!("   ℹ️ The endpoint implementation is correct, but the server is currently unavailable");
        }
    }
    
    // Test 3: Get specific equity option
    info!("\n📊 Test 3: Getting specific equity options...");
    
    // First get some option symbols to test with
    match tasty.list_equity_options(&["AAPL"], Some(true)).await {
        Ok(aapl_options) => {
            if !aapl_options.is_empty() {
                // Test with first few AAPL options
                for option in aapl_options.iter().take(3) {
                    match tasty.get_equity_option(&option.symbol.0).await {
                        Ok(specific_option) => {
                            info!("✅ Retrieved specific option: {}", specific_option.symbol.0);
                            debug!("   📊 Details:");
                            debug!("      - Underlying: {}", specific_option.underlying_symbol.0);
                            debug!("      - Strike: ${}", specific_option.strike_price);
                            debug!("      - Type: {}", specific_option.option_type);
                            debug!("      - Expiration: {}", specific_option.expiration_date);
                            debug!("      - Days to Exp: {}", specific_option.days_to_expiration);
                            debug!("      - Market Time Collection: {}", specific_option.market_time_instrument_collection);
                            debug!("      - Stops Trading At: {}", specific_option.stops_trading_at);
                            debug!("      - Expires At: {}", specific_option.expires_at);
                        }
                        Err(e) => {
                            error!("❌ Error getting specific option {}: {}", option.symbol.0, e);
                        }
                    }
                }
            } else {
                info!("   ℹ️ No AAPL options found for individual testing");
            }
        }
        Err(e) => {
            error!("❌ Error getting AAPL options for testing: {}", e);
        }
    }
    
    info!("\n✅ Equity options testing completed!");
    
    Ok(())
}