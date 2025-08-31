/******************************************************************************
    Author: Joaquín Béjar García
    Email: jb@taunais.com 
    Date: 31/8/25
 ******************************************************************************/
use std::collections::HashSet;
use chrono::{DateTime, Utc};
use tracing::{error, info};
use crate::prelude::{parse_expiration_date, SymbolEntry, TastyTradeConfig};
use crate::{InstrumentType, Symbol, TastyTrade};

/// Downloads all FutureOption and EquityOption symbols from TastyTrade
pub async fn download_options_symbols() -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
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

    let mut all_symbols = Vec::new();
    let now = Utc::now();

    // Download EquityOptions
    info!("📈 Downloading EquityOption symbols...");
    match download_equity_options(&tasty, now).await {
        Ok(mut equity_options) => {
            info!("✅ Downloaded {} EquityOption symbols", equity_options.len());
            all_symbols.append(&mut equity_options);
        }
        Err(e) => {
            error!("⚠️  Error downloading EquityOptions: {}", e);
        }
    }

    // Download FutureOptions
    info!("🔮 Downloading FutureOption symbols...");
    match download_future_options(&tasty, now).await {
        Ok(mut future_options) => {
            info!("✅ Downloaded {} FutureOption symbols", future_options.len());
            all_symbols.append(&mut future_options);
        }
        Err(e) => {
            error!("⚠️  Error downloading FutureOptions: {}", e);
        }
    }

    // Remove duplicates using HashSet
    let unique_symbols: HashSet<SymbolEntry> = all_symbols.into_iter().collect();
    let final_symbols: Vec<SymbolEntry> = unique_symbols.into_iter().collect();

    info!("🎯 Total unique symbols downloaded: {}", final_symbols.len());

    Ok(final_symbols)
}

/// Downloads EquityOption symbols from TastyTrade
async fn download_equity_options(
    tasty: &TastyTrade,
    last_update: DateTime<Utc>,
) -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
    let mut symbols = Vec::new();

    // Get a sample of equity symbols first to find their options
    // Note: In a real implementation, you might want to get all equities
    // or use a predefined list of popular symbols
    let sample_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "AMZN", "NVDA", "META", "NFLX"];

    for equity_symbol in sample_symbols {
        info!("  📊 Processing options for {}", equity_symbol);

        // Get nested option chains for this equity
        match tasty.list_nested_option_chains(Symbol(equity_symbol.to_string())).await {
            Ok(option_chains) => {
                for chain in option_chains {
                    // Process each expiration in the chain
                    for expiration in &chain.expirations {
                        // Parse expiration date
                        let expiry = parse_expiration_date(&expiration.expiration_date, last_update);

                        // Process each strike in the expiration
                        for strike in &expiration.strikes {
                            // Add call option
                            symbols.push(SymbolEntry {
                                symbol: strike.call.0.clone(),
                                epic: strike.call.0.clone(), // Using symbol as epic for TastyTrade
                                name: format!("{} Call ${} {}",
                                              chain.underlying_symbol.0,
                                              strike.strike_price,
                                              expiration.expiration_date
                                ),
                                instrument_type: InstrumentType::EquityOption,
                                exchange: "TASTYTRADE".to_string(),
                                expiry,
                                last_update,
                            });

                            // Add put option
                            symbols.push(SymbolEntry {
                                symbol: strike.put.0.clone(),
                                epic: strike.put.0.clone(), // Using symbol as epic for TastyTrade
                                name: format!("{} Put ${} {}",
                                              chain.underlying_symbol.0,
                                              strike.strike_price,
                                              expiration.expiration_date
                                ),
                                instrument_type: InstrumentType::EquityOption,
                                exchange: "TASTYTRADE".to_string(),
                                expiry,
                                last_update,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                error!("    ⚠️  Error getting option chain for {}: {}", equity_symbol, e);
            }
        }
    }

    Ok(symbols)
}

/// Downloads FutureOption symbols from TastyTrade
async fn download_future_options(
    tasty: &TastyTrade,
    last_update: DateTime<Utc>,
) -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
    let mut symbols = Vec::new();

    // Get future products first
    info!("  📈 Getting future products...");
    let future_products = tasty.list_future_products().await?;

    // Process a sample of future products (limit to avoid too many API calls)
    let sample_products: Vec<_> = future_products.into_iter().take(5).collect();

    for product in sample_products {
        info!("  🔮 Processing future options for product: {}", product.code);

        // Get nested option chains for this future product
        match tasty.list_nested_futures_option_chains(&product.code).await {
            Ok(option_chains) => {
                for chain in option_chains {
                    // Process each expiration in the chain
                    for expiration in &chain.expirations {
                        // Parse expiration date
                        let expiry = parse_expiration_date(&expiration.expiration_date, last_update);

                        // Process each strike in the expiration
                        for strike in &expiration.strikes {
                            // Add call option
                            symbols.push(SymbolEntry {
                                symbol: strike.call.0.clone(),
                                epic: strike.call.0.clone(), // Using symbol as epic for TastyTrade
                                name: format!("{} Future Call ${} {}",
                                              chain.underlying_symbol.0,
                                              strike.strike_price,
                                              expiration.expiration_date
                                ),
                                instrument_type: InstrumentType::FutureOption,
                                exchange: "TASTYTRADE".to_string(),
                                expiry,
                                last_update,
                            });

                            // Add put option
                            symbols.push(SymbolEntry {
                                symbol: strike.put.0.clone(),
                                epic: strike.put.0.clone(), // Using symbol as epic for TastyTrade
                                name: format!("{} Future Put ${} {}",
                                              chain.underlying_symbol.0,
                                              strike.strike_price,
                                              expiration.expiration_date
                                ),
                                instrument_type: InstrumentType::FutureOption,
                                exchange: "TASTYTRADE".to_string(),
                                expiry,
                                last_update,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                error!("    ⚠️  Error getting future option chain for {}: {}", product.code, e);
            }
        }
    }

    Ok(symbols)
}
