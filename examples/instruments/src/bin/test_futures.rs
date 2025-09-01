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

    info!("🚀 Testing futures endpoints");
    info!("=============================");

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

    // Test 1: List all futures without filters
    info!("\n📊 Test 1: Listing all futures (no filters)...");

    match tasty
        .list_futures(None::<&[&str]>, None, None, None, None)
        .await
    {
        Ok(futures) => {
            info!("✅ Found {} futures total", futures.len());

            if !futures.is_empty() {
                // Show first few futures
                for (i, future) in futures.iter().enumerate().take(5) {
                    debug!(
                        "   {}. {} | Product: {} | Exp: {} | Exchange: {} | Active: {}",
                        i + 1,
                        future.symbol.0,
                        future.product_code,
                        future.expiration_date,
                        future.exchange,
                        future.active
                    );

                    if i < 2 {
                        debug!("      - Contract Size: {}", future.contract_size);
                        debug!("      - Tick Size: {}", future.tick_size);
                        debug!(
                            "      - Notional Multiplier: {}",
                            future.notional_multiplier
                        );
                        debug!("      - Display Factor: {}", future.display_factor);
                        debug!("      - Last Trade Date: {}", future.last_trade_date);
                        debug!("      - Product Group: {}", future.product_group);
                        debug!("      - Active Month: {}", future.active_month);
                        debug!("      - Next Active Month: {}", future.next_active_month);
                        debug!("      - Is Closing Only: {}", future.is_closing_only);
                        debug!("      - Is Tradeable: {}", future.is_tradeable);
                        if let Some(closing_date) = &future.closing_only_date {
                            debug!("      - Closing Only Date: {}", closing_date);
                        }
                    }
                }

                if futures.len() > 5 {
                    debug!("   ... and {} more futures", futures.len() - 5);
                }

                // Analyze futures by various criteria
                let active = futures.iter().filter(|f| f.active).count();
                let active_month = futures.iter().filter(|f| f.active_month).count();
                let next_active_month = futures.iter().filter(|f| f.next_active_month).count();
                let closing_only = futures.iter().filter(|f| f.is_closing_only).count();
                let tradeable = futures.iter().filter(|f| f.is_tradeable).count();

                info!("   📈 Analysis:");
                debug!("      - Active: {}", active);
                debug!("      - Active Month: {}", active_month);
                debug!("      - Next Active Month: {}", next_active_month);
                debug!("      - Closing Only: {}", closing_only);
                debug!("      - Tradeable: {}", tradeable);

                // Group by product code
                let mut product_count = std::collections::HashMap::new();
                for future in &futures {
                    *product_count
                        .entry(future.product_code.clone())
                        .or_insert(0) += 1;
                }

                debug!("   📊 Top 10 products by future count:");
                let mut sorted_products: Vec<_> = product_count.iter().collect();
                sorted_products.sort_by(|a, b| b.1.cmp(a.1));

                for (i, (product, count)) in sorted_products.iter().take(10).enumerate() {
                    debug!("      {}. {}: {} futures", i + 1, product, count);
                }

                // Group by exchange
                let mut exchange_count = std::collections::HashMap::new();
                for future in &futures {
                    *exchange_count.entry(future.exchange.clone()).or_insert(0) += 1;
                }

                debug!("   📊 Futures by exchange:");
                for (exchange, count) in exchange_count.iter() {
                    debug!("      - {}: {} futures", exchange, count);
                }

                // Group by product group
                let mut group_count = std::collections::HashMap::new();
                for future in &futures {
                    *group_count.entry(future.product_group.clone()).or_insert(0) += 1;
                }

                debug!("   📊 Futures by product group:");
                let mut sorted_groups: Vec<_> = group_count.iter().collect();
                sorted_groups.sort_by(|a, b| b.1.cmp(a.1));

                for (i, (group, count)) in sorted_groups.iter().take(5).enumerate() {
                    debug!("      {}. {}: {} futures", i + 1, group, count);
                }
            }
        }
        Err(e) => {
            error!("❌ Error listing all futures: {}", e);
        }
    }

    // Test 2: List futures by product codes
    info!("\n📊 Test 2: Listing futures by product codes...");

    let popular_products = vec!["ES", "NQ", "YM", "RTY", "CL", "GC", "SI"];

    for product_code in popular_products {
        match tasty
            .list_futures(None::<&[&str]>, Some(product_code), None, None, None)
            .await
        {
            Ok(futures) => {
                info!(
                    "✅ Found {} futures for product {}",
                    futures.len(),
                    product_code
                );

                if !futures.is_empty() {
                    // Show details for this product
                    let active = futures.iter().filter(|f| f.active).count();
                    let active_month = futures.iter().filter(|f| f.active_month).count();

                    debug!(
                        "   📊 {} analysis: {} active, {} active month",
                        product_code, active, active_month
                    );

                    // Show first few futures for this product
                    for (i, future) in futures.iter().enumerate().take(3) {
                        debug!(
                            "      {}. {} - exp: {} (active: {}, tradeable: {})",
                            i + 1,
                            future.symbol.0,
                            future.expiration_date,
                            future.active,
                            future.is_tradeable
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "❌ Error getting futures for product {}: {}",
                    product_code, e
                );
            }
        }
    }

    // Test 3: Get specific futures by symbols
    info!("\n📊 Test 3: Getting specific futures by symbols...");

    // First get some ES futures to test with
    match tasty
        .list_futures(None::<&[&str]>, Some("ES"), None, None, None)
        .await
    {
        Ok(es_futures) => {
            if !es_futures.is_empty() {
                let test_symbols: Vec<_> = es_futures.iter().take(3).map(|f| &f.symbol).collect();

                match tasty
                    .list_futures(Some(&test_symbols), None, None, None, None)
                    .await
                {
                    Ok(specific_futures) => {
                        info!(
                            "✅ Retrieved {} specific futures by symbols",
                            specific_futures.len()
                        );

                        for future in &specific_futures {
                            debug!(
                                "   📊 {}: {} - {}",
                                future.symbol.0, future.product_code, future.expiration_date
                            );
                        }
                    }
                    Err(e) => {
                        error!("❌ Error getting specific futures by symbols: {}", e);
                    }
                }

                // Test 4: Get individual future details
                info!("\n📊 Test 4: Getting individual future details...");

                for future in es_futures.iter().take(2) {
                    match tasty.get_future(&future.symbol.0).await {
                        Ok(specific_future) => {
                            info!(
                                "✅ Retrieved future details for {}",
                                specific_future.symbol.0
                            );
                            debug!("   📊 Full details:");
                            debug!("      - Product Code: {}", specific_future.product_code);
                            debug!("      - Contract Size: {}", specific_future.contract_size);
                            debug!("      - Tick Size: {}", specific_future.tick_size);
                            debug!(
                                "      - Notional Multiplier: {}",
                                specific_future.notional_multiplier
                            );
                            debug!("      - Main Fraction: {}", specific_future.main_fraction);
                            debug!("      - Sub Fraction: {}", specific_future.sub_fraction);
                            debug!("      - Display Factor: {}", specific_future.display_factor);
                            debug!(
                                "      - Last Trade Date: {}",
                                specific_future.last_trade_date
                            );
                            debug!(
                                "      - Expiration Date: {}",
                                specific_future.expiration_date
                            );
                            debug!(
                                "      - Stops Trading At: {}",
                                specific_future.stops_trading_at
                            );
                            debug!("      - Expires At: {}", specific_future.expires_at);
                            debug!("      - Product Group: {}", specific_future.product_group);
                            debug!("      - Exchange: {}", specific_future.exchange);
                            debug!(
                                "      - Streamer Exchange Code: {}",
                                specific_future.streamer_exchange_code
                            );
                            debug!(
                                "      - Streamer Symbol: {}",
                                specific_future.streamer_symbol.0
                            );
                            debug!(
                                "      - Back Month First Calendar: {}",
                                specific_future.back_month_first_calendar_symbol
                            );
                            debug!("      - Is Tradeable: {}", specific_future.is_tradeable);

                            if let Some(roll_target) = &specific_future.roll_target_symbol {
                                debug!("      - Roll Target Symbol: {}", roll_target.0);
                            }

                            // Show future product details
                            debug!("   📊 Future Product:");
                            debug!(
                                "      - Root Symbol: {}",
                                specific_future.future_product.root_symbol.0
                            );
                            debug!("      - Code: {}", specific_future.future_product.code);
                            debug!(
                                "      - Description: {}",
                                specific_future.future_product.description
                            );
                            debug!(
                                "      - Exchange: {}",
                                specific_future.future_product.exchange
                            );
                            debug!(
                                "      - Product Type: {}",
                                specific_future.future_product.product_type
                            );
                            debug!(
                                "      - Market Sector: {}",
                                specific_future.future_product.market_sector
                            );
                            debug!(
                                "      - Cash Settled: {}",
                                specific_future.future_product.cash_settled
                            );
                            debug!(
                                "      - Small Notional: {}",
                                specific_future.future_product.small_notional
                            );
                        }
                        Err(e) => {
                            error!(
                                "❌ Error getting future details for {}: {}",
                                future.symbol.0, e
                            );
                        }
                    }
                }
            } else {
                info!("   ℹ️ No ES futures found for individual testing");
            }
        }
        Err(e) => {
            error!("❌ Error getting ES futures for testing: {}", e);
        }
    }

    info!("\n✅ Futures testing completed!");

    Ok(())
}
