/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    println!("🚀 Testing list_futures method");
    println!("==============================");

    // Load configuration from environment
    let config = TastyTradeConfig::from_env();

    // Check if we have valid credentials
    if !config.has_valid_credentials() {
        eprintln!(
            "❌ No valid credentials found. Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables."
        );
        return Err("Missing credentials".into());
    }

    println!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::login(&config).await?;
    println!("✅ Successfully logged in!");

    println!();
    println!("📊 Testing different approaches to get futures...");

    // Test 1: Get all futures (no filters)
    println!("\n📄 ==================== TEST 1: ALL FUTURES ====================");
    println!("📊 Getting all futures with no filters...");

    match tasty.list_futures(None::<&[&str]>, None, None, None, None).await {
        Ok(futures) => {
            let futures_count = futures.len();
            println!("✅ Found {} total futures", futures_count);

            if futures_count > 0 {
                println!("📋 FUTURES FOUND:");
                for (i, future) in futures.iter().enumerate().take(10) {
                    println!(
                        "   {}. Symbol: {} | Product: {} | Exchange: {} | Active: {} | Expiration: {}",
                        i + 1,
                        future.symbol.0,
                        future.product_code,
                        future.exchange,
                        future.active,
                        future.expiration_date
                    );

                    // Show additional details for first few items
                    if i < 3 {
                        println!("      - Contract Size: {}", future.contract_size);
                        println!("      - Tick Size: {}", future.tick_size);
                        println!("      - Product Group: {}", future.product_group);
                        println!("      - Active Month: {}", future.active_month);
                        println!("      - Is Tradeable: {}", future.is_tradeable);
                    }
                }

                if futures_count > 10 {
                    println!("   ... and {} more futures", futures_count - 10);
                }

                // Group by product code for analysis
                let mut product_count = std::collections::HashMap::new();
                for future in &futures {
                    *product_count
                        .entry(future.product_code.clone())
                        .or_insert(0) += 1;
                }

                println!("\n📊 Analysis:");
                println!("   • Unique product codes: {}", product_count.len());

                // Show top 10 product codes with most futures
                let mut sorted_products: Vec<_> = product_count.iter().collect();
                sorted_products.sort_by(|a, b| b.1.cmp(a.1));

                println!("   📈 Top 10 product codes with most futures:");
                for (i, (product, count)) in sorted_products.iter().take(10).enumerate() {
                    println!("      {}. {} - {} futures", i + 1, product, count);
                }

                // Group by exchange
                let mut exchange_count = std::collections::HashMap::new();
                for future in &futures {
                    *exchange_count.entry(future.exchange.clone()).or_insert(0) += 1;
                }

                println!("   📈 Futures by exchange:");
                for (exchange, count) in exchange_count.iter() {
                    println!("      - {}: {} futures", exchange, count);
                }

                // Count active vs inactive
                let active_count = futures.iter().filter(|f| f.active).count();
                let inactive_count = futures_count - active_count;
                println!("   📈 Status breakdown:");
                println!("      - Active: {} futures", active_count);
                println!("      - Inactive: {} futures", inactive_count);
            } else {
                println!("❌ NO FUTURES FOUND");
            }
        }
        Err(e) => {
            println!("❌ Error getting all futures: {}", e);
        }
    }

    // Test 2: Get futures by specific product codes
    println!("\n📄 ==================== TEST 2: BY PRODUCT CODE ====================");
    let popular_products = vec!["ES", "NQ", "YM", "RTY", "CL", "GC", "SI"];

    for product_code in popular_products {
        println!("📊 Getting futures for product code: {}", product_code);

        match tasty
            .list_futures(None::<&[&str]>, Some(product_code), None, None, None)
            .await
        {
            Ok(futures) => {
                println!(
                    "   ✅ Found {} futures for product {}",
                    futures.len(),
                    product_code
                );

                if !futures.is_empty() {
                    // Show first few futures for this product
                    for (i, future) in futures.iter().enumerate().take(3) {
                        println!(
                            "      {}. {} | Exp: {} | Active: {} | Tradeable: {}",
                            i + 1,
                            future.symbol.0,
                            future.expiration_date,
                            future.active,
                            future.is_tradeable
                        );
                    }
                    if futures.len() > 3 {
                        println!("      ... and {} more", futures.len() - 3);
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Error getting futures for {}: {}", product_code, e);
            }
        }
    }

    // Test 3: Get futures by specific symbols (if we found any)
    println!("\n📄 ==================== TEST 3: BY SYMBOLS ====================");

    // First get some symbols to test with
    match tasty.list_futures(None::<&[&str]>, Some("ES"), None, None, None).await {
        Ok(es_futures) => {
            if !es_futures.is_empty() {
                let test_symbols: Vec<_> = es_futures.iter().take(3).map(|f| &f.symbol).collect();
                println!(
                    "📊 Testing with specific symbols: {:?}",
                    test_symbols.iter().map(|s| &s.0).collect::<Vec<_>>()
                );

                match tasty.list_futures(Some(&test_symbols), None, None, None, None).await {
                    Ok(symbol_futures) => {
                        println!(
                            "   ✅ Found {} futures for specific symbols",
                            symbol_futures.len()
                        );
                        for future in symbol_futures {
                            println!(
                                "      - {} | Product: {} | Exp: {} | Active: {}",
                                future.symbol.0,
                                future.product_code,
                                future.expiration_date,
                                future.active
                            );
                        }
                    }
                    Err(e) => {
                        println!("   ❌ Error getting futures by symbols: {}", e);
                    }
                }
            } else {
                println!("📊 No ES futures found to test with symbols");
            }
        }
        Err(e) => {
            println!("📊 Could not get ES futures for symbol test: {}", e);
        }
    }

    println!();
    println!("✅ Testing completed!");

    Ok(())
}
