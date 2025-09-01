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

    info!("🚀 Testing future option products endpoints");
    info!("=============================================");

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

    // Test 1: List all future option products
    info!("\n📊 Test 1: Listing all future option products...");
    match tasty.list_future_option_products().await {
        Ok(products) => {
            info!("✅ Found {} future option products", products.len());

            if !products.is_empty() {
                // Show first few products
                for (i, product) in products.iter().enumerate().take(5) {
                    debug!(
                        "   {}. {} | Exchange: {} | Code: {} | Cash Settled: {}",
                        i + 1,
                        product.root_symbol,
                        product.exchange,
                        product.code,
                        product.cash_settled
                    );

                    if i < 2 {
                        debug!("      - Product Type: {}", product.product_type);
                        debug!("      - Expiration Type: {}", product.expiration_type);
                        debug!("      - Market Sector: {}", product.market_sector);
                        debug!("      - Display Factor: {}", product.display_factor);
                    }
                }

                if products.len() > 5 {
                    debug!("   ... and {} more products", products.len() - 5);
                }

                // Analyze by exchange
                let mut exchange_count = std::collections::HashMap::new();
                for product in &products {
                    *exchange_count.entry(product.exchange.clone()).or_insert(0) += 1;
                }

                info!("   📈 Products by exchange:");
                for (exchange, count) in exchange_count.iter() {
                    debug!("      - {}: {} products", exchange, count);
                }

                // Analyze by product type
                let mut type_count = std::collections::HashMap::new();
                for product in &products {
                    *type_count.entry(product.product_type.clone()).or_insert(0) += 1;
                }

                info!("   📊 Products by type:");
                for (prod_type, count) in type_count.iter() {
                    debug!("      - {}: {} products", prod_type, count);
                }

                // Test 2: Get specific products by exchange and root symbol
                info!("\n📊 Test 2: Getting specific products by exchange and root symbol...");

                // Test with first few products
                for product in products.iter().take(3) {
                    match tasty
                        .get_future_option_product_by_exchange(
                            &product.exchange,
                            &product.root_symbol,
                        )
                        .await
                    {
                        Ok(specific_product) => {
                            info!(
                                "✅ Retrieved product {} from exchange {}",
                                specific_product.root_symbol, specific_product.exchange
                            );
                            debug!("   📊 Product details:");
                            debug!("      - Code: {}", specific_product.code);
                            debug!("      - Cash Settled: {}", specific_product.cash_settled);
                            debug!(
                                "      - Settlement Delay: {} days",
                                specific_product.settlement_delay_days
                            );
                            debug!("      - Is Rollover: {}", specific_product.is_rollover);
                        }
                        Err(e) => {
                            error!(
                                "❌ Error getting product {} from {}: {}",
                                product.root_symbol, product.exchange, e
                            );
                        }
                    }
                }

                // Test 3: Get products by root symbol only
                info!("\n📊 Test 3: Getting products by root symbol only...");

                // Test with first few unique root symbols
                let mut tested_symbols = std::collections::HashSet::new();
                for product in products.iter() {
                    if tested_symbols.len() >= 3 {
                        break;
                    }

                    if tested_symbols.insert(product.root_symbol.clone()) {
                        match tasty.get_future_option_product(&product.root_symbol).await {
                            Ok(root_product) => {
                                info!(
                                    "✅ Retrieved product by root symbol: {}",
                                    root_product.root_symbol
                                );
                                debug!("   📊 Root product details:");
                                debug!("      - Exchange: {}", root_product.exchange);
                                debug!("      - Legacy Code: {}", root_product.legacy_code);
                                debug!("      - Clearing Code: {}", root_product.clearing_code);
                                debug!(
                                    "      - Clearing Exchange: {}",
                                    root_product.clearing_exchange_code
                                );
                            }
                            Err(e) => {
                                error!(
                                    "❌ Error getting product by root symbol {}: {}",
                                    product.root_symbol, e
                                );
                            }
                        }
                    }
                }
            } else {
                info!("   ℹ️ No future option products found");
            }
        }
        Err(e) => {
            error!("❌ Error listing future option products: {}", e);
        }
    }

    info!("\n✅ Future option products testing completed!");

    Ok(())
}