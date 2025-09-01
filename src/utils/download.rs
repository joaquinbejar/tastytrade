/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/
use crate::prelude::{SymbolEntry, TastyTradeConfig, parse_expiration_date};
use crate::{InstrumentType, TastyTrade};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use tracing::{error, info};

/// Downloads all FutureOption and EquityOption symbols from TastyTrade
pub async fn download_options_symbols() -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
    // Load configuration from environment
    let config = TastyTradeConfig::new();

    // Check if we have valid credentials
    if !config.has_valid_credentials() {
        error!(
            "❌ No valid credentials found. Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables."
        );
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
            info!(
                "✅ Downloaded {} EquityOption symbols",
                equity_options.len()
            );
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
            info!(
                "✅ Downloaded {} FutureOption symbols",
                future_options.len()
            );
            all_symbols.append(&mut future_options);
        }
        Err(e) => {
            error!("⚠️  Error downloading FutureOptions: {}", e);
        }
    }

    // Remove duplicates using HashSet
    let unique_symbols: HashSet<SymbolEntry> = all_symbols.into_iter().collect();
    let final_symbols: Vec<SymbolEntry> = unique_symbols.into_iter().collect();

    info!(
        "🎯 Total unique symbols downloaded: {}",
        final_symbols.len()
    );

    Ok(final_symbols)
}

/// Downloads EquityOption symbols from TastyTrade
async fn download_equity_options(
    tasty: &TastyTrade,
    last_update: DateTime<Utc>,
) -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
    let mut symbols = Vec::new();

    // Try different approaches to get equity symbols
    info!("  📊 Getting equity symbols using multiple approaches...");
    let mut all_equities = Vec::new();

    // Approach 1: Try to get active equities with pagination
    info!("  📊 Trying list_active_equities...");
    let max_pages = 5; // Limit to avoid infinite loops

    for page in 0..max_pages {
        match tasty.list_active_equities(page).await {
            Ok(paginated_equities) => {
                let current_count = paginated_equities.items.len();
                info!("    📄 Page {}: {} items found", page, current_count);

                // Check pagination info first
                let pagination = &paginated_equities.pagination;

                // Debug: Print full response structure
                info!("    🔍 DEBUG - Full response for page {}:", page);
                info!("    🔍 Items count: {}", current_count);

                // Print ALL items in this page
                for (i, item) in paginated_equities.items.iter().enumerate() {
                    info!(
                        "    🔍 Item {}: symbol={}, id={}, active={}, description={}",
                        i, item.symbol.0, item.id, item.active, item.description
                    );
                }

                if current_count == 0 {
                    info!(
                        "    🔍 ⚠️  PAGE {} IS EMPTY - but API says there are {} total items",
                        page, pagination.total_items
                    );
                }
                info!(
                    "    📊 Pagination: page {}/{}, total items: {}",
                    pagination.page_offset, pagination.total_pages, pagination.total_items
                );
                info!(
                    "    🔍 DEBUG - Pagination details: per_page={}, item_offset={}, current_item_count={}",
                    pagination.per_page, pagination.item_offset, pagination.current_item_count
                );

                if current_count > 0 {
                    all_equities.extend(paginated_equities.items);
                }

                // Break if we've reached the last page
                if pagination.page_offset + 1 >= pagination.total_pages {
                    break;
                }

                // If we have total_items but no items on this page, continue to next page
                if current_count == 0 && pagination.total_items > 0 {
                    info!(
                        "    📄 Empty page but {} total items exist, continuing...",
                        pagination.total_items
                    );
                    continue;
                }

                // If no items and no total items, we're done
                if current_count == 0 && pagination.total_items == 0 {
                    break;
                }
            }
            Err(e) => {
                error!("Error fetching active equities at page {}: {}", page, e);
                break;
            }
        }
    }

    // If we didn't get any equities, there's a problem that needs investigation
    if all_equities.is_empty() {
        error!("  ❌ No equity instruments found via list_active_equities API");
        error!("  🔍 This indicates a potential API issue or authentication problem");
        return Err("No equity instruments found - check API connectivity and credentials".into());
    }

    info!("  📊 Found {} total equity instruments", all_equities.len());

    // Process options for each equity (limit to avoid overwhelming API)
    let max_equities = std::env::var("MAX_EQUITIES")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()
        .unwrap_or(100);

    let equities_to_process = if all_equities.len() > max_equities {
        info!(
            "  ⚠️  Limiting to {} equities (set MAX_EQUITIES env var to change)",
            max_equities
        );
        &all_equities[..max_equities]
    } else {
        &all_equities
    };

    for equity in equities_to_process {
        info!("  📊 Processing options for {}", equity.symbol.0);

        // Get nested option chains for this equity
        match tasty.list_nested_option_chains(equity.symbol.clone()).await {
            Ok(option_chains) => {
                for chain in option_chains {
                    // Process each expiration in the chain
                    for expiration in &chain.expirations {
                        // Parse expiration date
                        let expiry =
                            parse_expiration_date(&expiration.expiration_date, last_update);

                        // Process each strike in the expiration
                        for strike in &expiration.strikes {
                            // Add call option
                            symbols.push(SymbolEntry {
                                symbol: strike.call.0.clone(),
                                epic: strike.call.0.clone(), // Using symbol as epic for TastyTrade
                                name: format!(
                                    "{} Call ${} {}",
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
                                name: format!(
                                    "{} Put ${} {}",
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
                error!(
                    "    ⚠️  Error getting option chain for {}: {}",
                    equity.symbol.0, e
                );
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

    // Get ALL future products
    info!("  📈 Getting all future products...");
    let future_products = tasty.list_future_products().await?;

    info!("  📈 Found {} total future products", future_products.len());

    // Process all future products (with optional limit via env var)
    let max_products = std::env::var("MAX_FUTURE_PRODUCTS")
        .unwrap_or_else(|_| "50".to_string())
        .parse::<usize>()
        .unwrap_or(50);

    let products_to_process = if future_products.len() > max_products {
        info!(
            "  ⚠️  Limiting to {} future products (set MAX_FUTURE_PRODUCTS env var to change)",
            max_products
        );
        &future_products[..max_products]
    } else {
        &future_products
    };

    // Products that typically don't have option chains
    let products_without_options = [
        "GE", // Eurodollar
        "ZQ", // 30 Day Fed Fund
        "ZT", // 2-Year Note
        "ZF", // 5-Year Note
        "ZN", // 10-Year Note
        "ZB", // 30-Year Bond
        "UB",
    ];

    for product in products_to_process {
        info!(
            "  🔮 Processing future options for product: {} ({})",
            product.code, product.description
        );

        // Skip products that typically don't have option chains
        if products_without_options.contains(&product.code.as_str()) {
            info!(
                "    📝 {} ({}) typically has no option chains - skipping",
                product.code, product.description
            );
            continue;
        }

        // Get nested option chains for this future product
        match tasty.list_nested_futures_option_chains(&product.code).await {
            Ok(option_chains) => {
                if option_chains.is_empty() {
                    info!(
                        "    📭 No option chains found for {} ({})",
                        product.code, product.description
                    );
                    continue;
                }
                info!(
                    "    ✅ Found {} option chains for {}",
                    option_chains.len(),
                    product.code
                );
                for chain in option_chains {
                    // Process each option chain in the nested structure
                    for option_chain in &chain.option_chains {
                        // Process each expiration in the chain
                        for expiration in &option_chain.expirations {
                            // Parse expiration date
                            let expiry =
                                parse_expiration_date(&expiration.expiration_date, last_update);

                            // Process each strike in the expiration
                            for strike in &expiration.strikes {
                                // Add call option
                                symbols.push(SymbolEntry {
                                    symbol: strike.call.clone(),
                                    epic: strike.call.clone(), // Using symbol as epic for TastyTrade
                                    name: format!(
                                        "{} Future Call ${} {}",
                                        option_chain.underlying_symbol,
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
                                     symbol: strike.put.clone(),
                                     epic: strike.put.clone(), // Using symbol as epic for TastyTrade
                                     name: format!(
                                         "{} Future Put ${} {}",
                                         option_chain.underlying_symbol,
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
            }
            Err(e) => {
                // Check if it's a decoding error specifically
                let error_msg = format!("{}", e);
                if error_msg.contains("error decoding response body") {
                    info!(
                        "    📝 {} ({}) has no option chains or unsupported format - skipping",
                        product.code, product.description
                    );
                } else {
                    error!(
                        "    ⚠️  API error for {} ({}): {}",
                        product.code, product.description, e
                    );
                }
            }
        }
    }

    Ok(symbols)
}
