/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    info!("🚀 Testing option chains endpoints");
    info!("====================================");

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

    // Test popular equity symbols
    let popular_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "SPY"];

    for symbol in popular_symbols {
        info!("\n📊 Testing option chains for symbol: {}", symbol);

        // Test 1: Get standard option chain
        match tasty.list_option_chains(symbol).await {
            Ok(options) => {
                info!(
                    "✅ Found {} equity options for symbol {}",
                    options.len(),
                    symbol
                );

                if !options.is_empty() {
                    // Show first few options
                    for (i, option) in options.iter().enumerate().take(3) {
                        debug!(
                            "   {}. {} | Strike: ${} | Exp: {} | Type: {} | Active: {}",
                            i + 1,
                            option.symbol.0,
                            option.strike_price,
                            option.expiration_date,
                            option.option_type,
                            option.active
                        );
                    }
                    if options.len() > 3 {
                        debug!("   ... and {} more options", options.len() - 3);
                    }

                    // Analyze option types
                    let calls = options.iter().filter(|o| o.option_type == "C").count();
                    let puts = options.iter().filter(|o| o.option_type == "P").count();
                    let active = options.iter().filter(|o| o.active).count();

                    info!(
                        "   📈 Analysis: {} calls, {} puts, {} active",
                        calls, puts, active
                    );

                    // Group by expiration
                    let mut expirations = std::collections::HashMap::new();
                    for option in &options {
                        *expirations
                            .entry(option.expiration_date.clone())
                            .or_insert(0) += 1;
                    }

                    info!("   📅 Found {} unique expiration dates", expirations.len());

                    // Show top 3 expirations by option count
                    let mut sorted_exps: Vec<_> = expirations.iter().collect();
                    sorted_exps.sort_by(|a, b| b.1.cmp(a.1));

                    for (i, (exp_date, count)) in sorted_exps.iter().take(3).enumerate() {
                        debug!("      {}. {} - {} options", i + 1, exp_date, count);
                    }
                } else {
                    info!("   ℹ️ No options found for symbol {}", symbol);
                }
            }
            Err(e) => {
                error!("❌ Error getting option chain for {}: {}", symbol, e);
            }
        }

        // Test 2: Get compact option chain
        match tasty.get_compact_option_chain(symbol).await {
            Ok(compact_chain) => {
                info!("✅ Retrieved compact option chain for {}", symbol);
                debug!("   📊 Compact chain details:");
                debug!("      - Underlying: {}", compact_chain.underlying_symbol.0);
                debug!("      - Root: {}", compact_chain.root_symbol.0);
                debug!("      - Chain Type: {}", compact_chain.option_chain_type);
                debug!(
                    "      - Shares per Contract: {}",
                    compact_chain.shares_per_contract
                );

                if let Some(settlement) = &compact_chain.settlement_type {
                    debug!("      - Settlement Type: {}", settlement);
                }

                if let Some(exp_type) = &compact_chain.expiration_type {
                    debug!("      - Expiration Type: {}", exp_type);
                }

                if let Some(symbols) = &compact_chain.symbols {
                    debug!(
                        "      - Symbols: {} symbols in compact format",
                        symbols.len()
                    );
                }
            }
            Err(e) => {
                error!(
                    "❌ Error getting compact option chain for {}: {}",
                    symbol, e
                );
            }
        }

        // Test 3: Compare with nested version
        match tasty.list_nested_option_chains(symbol).await {
            Ok(nested_chains) => {
                info!(
                    "✅ Found {} nested option chains for symbol {}",
                    nested_chains.len(),
                    symbol
                );

                if !nested_chains.is_empty() {
                    let total_options: usize = nested_chains
                        .iter()
                        .map(|chain| {
                            chain
                                .expirations
                                .iter()
                                .map(|exp| exp.strikes.len() * 2) // Each strike has call and put
                                .sum::<usize>()
                        })
                        .sum();

                    info!(
                        "   📊 Nested format contains ~{} total options across {} chains",
                        total_options,
                        nested_chains.len()
                    );

                    // Show expiration count per chain
                    for (i, chain) in nested_chains.iter().enumerate().take(2) {
                        debug!(
                            "      Chain {}: {} expirations, root: {}",
                            i + 1,
                            chain.expirations.len(),
                            chain.root_symbol.0
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "❌ Error getting nested option chains for {}: {}",
                    symbol, e
                );
            }
        }
    }

    info!("\n✅ Option chains testing completed!");

    Ok(())
}
