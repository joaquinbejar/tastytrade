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

    info!("🚀 Testing future options endpoints");
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

    // Get actual future symbols from future products
    info!("\n📊 Getting actual future symbols for testing...");

    let mut verified_symbols = Vec::new();

    // Get future products to find available product codes
    match tasty.list_future_products().await {
        Ok(products) => {
            info!("✅ Found {} future products", products.len());

            // Get futures for the first few products
            for product in products.iter().take(3) {
                info!(
                    "🔍 Getting futures for product: {} ({})",
                    product.code, product.description
                );

                match tasty
                    .list_futures(None::<&[&str]>, Some(&product.code), None, None, None)
                    .await
                {
                    Ok(futures) => {
                        if !futures.is_empty() {
                            info!(
                                "✅ Found {} futures for product {}",
                                futures.len(),
                                product.code
                            );

                            // Add first few futures from this product
                            for future in futures.iter().take(2) {
                                verified_symbols.push(future.symbol.0.clone());
                                info!(
                                    "   - Added future: {} (expires: {})",
                                    future.symbol.0, future.expiration_date
                                );
                            }
                        } else {
                            debug!("ℹ️ No futures found for product {}", product.code);
                        }
                    }
                    Err(e) => {
                        debug!("❌ Error getting futures for {}: {}", product.code, e);
                    }
                }

                // Stop if we have enough symbols
                if verified_symbols.len() >= 5 {
                    break;
                }
            }
        }
        Err(e) => {
            error!("❌ Error getting future products: {}", e);
        }
    }

    if verified_symbols.is_empty() {
        error!("❌ No future symbols could be obtained from products");
        return Err("No future symbols available for testing".into());
    }

    info!(
        "✅ Using {} verified future symbols for testing",
        verified_symbols.len()
    );

    // Demonstrate new list_futures parameters
    info!("\n🔧 Testing new list_futures parameters...");

    // Test with only active futures
    match tasty
        .list_futures(None::<&[&str]>, Some("MES"), None, Some(true), None)
        .await
    {
        Ok(active_futures) => {
            info!("✅ Found {} active MES futures", active_futures.len());
        }
        Err(e) => {
            debug!("❌ Error getting active MES futures: {}", e);
        }
    }

    // Test with exchange parameter
    match tasty
        .list_futures(None::<&[&str]>, Some("GE"), Some("CME"), None, None)
        .await
    {
        Ok(cme_futures) => {
            info!("✅ Found {} GE futures on CME exchange", cme_futures.len());
        }
        Err(e) => {
            debug!("❌ Error getting CME GE futures: {}", e);
        }
    }

    // Test 1: List futures option chains by product codes
    info!("\n📊 Test 1: Listing futures option chains by product codes...");

    // Get product codes from the futures we found
    let mut product_codes = Vec::new();
    for symbol in &verified_symbols {
        match tasty.get_future(symbol).await {
            Ok(future) => {
                if !product_codes.contains(&future.product_code) {
                    product_codes.push(future.product_code.clone());
                }
            }
            Err(e) => {
                debug!("Error getting future {}: {}", symbol, e);
            }
        }
    }

    info!(
        "📊 Found {} unique product codes: {:?}",
        product_codes.len(),
        product_codes
    );

    let mut all_options = Vec::new();

    for product_code in &product_codes {
        info!(
            "🔍 Getting futures option chains for product: {}",
            product_code
        );

        match tasty.list_futures_option_chains(product_code).await {
            Ok(options) => {
                info!(
                    "✅ Found {} future options for product {}",
                    options.len(),
                    product_code
                );
                all_options.extend(options);
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if error_msg.contains("404") || error_msg.contains("record_not_found") {
                    info!(
                        "ℹ️ No future options available for product {} (404 - not found)",
                        product_code
                    );
                } else {
                    error!(
                        "❌ Error getting future options for {}: {}",
                        product_code, e
                    );
                }
            }
        }
    }

    let options = all_options;

    if options.is_empty() {
        info!("ℹ️ No future options found for any product codes");
    } else {
        info!(
            "✅ Found {} future options for specified symbols",
            options.len()
        );

        if !options.is_empty() {
            // Show first few options
            for (i, option) in options.iter().enumerate().take(5) {
                debug!(
                    "   {}. {} | Strike: ${} | Exp: {} | Type: {} | Underlying: {}",
                    i + 1,
                    option.symbol.0,
                    option.strike_price,
                    option.expiration_date,
                    option.option_type,
                    option.underlying_symbol.0
                );

                if i < 2 {
                    debug!("      - Product Code: {}", option.product_code);
                    debug!("      - Root Symbol: {}", option.root_symbol.0);
                    debug!("      - Option Root Symbol: {}", option.option_root_symbol);
                    debug!("      - Exchange: {}", option.exchange);
                    debug!("      - Exercise Style: {}", option.exercise_style);
                    debug!("      - Is Vanilla: {}", option.is_vanilla);
                    debug!(
                        "      - Is Primary Deliverable: {}",
                        option.is_primary_deliverable
                    );
                    debug!("      - Days to Expiration: {}", option.days_to_expiration);
                    debug!("      - Active: {}", option.active);
                    debug!("      - Closing Only: {}", option.is_closing_only);
                }
            }

            if options.len() > 5 {
                debug!("   ... and {} more options", options.len() - 5);
            }

            // Analyze option types and characteristics
            let calls = options.iter().filter(|o| o.option_type == "C").count();
            let puts = options.iter().filter(|o| o.option_type == "P").count();
            let vanilla = options.iter().filter(|o| o.is_vanilla).count();
            let primary_deliverable = options.iter().filter(|o| o.is_primary_deliverable).count();
            let active = options.iter().filter(|o| o.active).count();
            let closing_only = options.iter().filter(|o| o.is_closing_only).count();

            info!("   📈 Analysis:");
            debug!("      - Calls: {}", calls);
            debug!("      - Puts: {}", puts);
            debug!("      - Vanilla: {}", vanilla);
            debug!("      - Primary Deliverable: {}", primary_deliverable);
            debug!("      - Active: {}", active);
            debug!("      - Closing Only: {}", closing_only);

            // Group by underlying
            let mut underlying_count = std::collections::HashMap::new();
            for option in &options {
                *underlying_count
                    .entry(option.underlying_symbol.0.clone())
                    .or_insert(0) += 1;
            }

            debug!("   📊 Options by underlying:");
            for (underlying, count) in underlying_count.iter() {
                debug!("      - {}: {} options", underlying, count);
            }

            // Group by product code
            let mut product_count = std::collections::HashMap::new();
            for option in &options {
                *product_count
                    .entry(option.product_code.clone())
                    .or_insert(0) += 1;
            }

            debug!("   📊 Options by product code:");
            for (product, count) in product_count.iter() {
                debug!("      - {}: {} options", product, count);
            }

            // Group by exchange
            let mut exchange_count = std::collections::HashMap::new();
            for option in &options {
                *exchange_count.entry(option.exchange.clone()).or_insert(0) += 1;
            }

            debug!("   📊 Options by exchange:");
            for (exchange, count) in exchange_count.iter() {
                debug!("      - {}: {} options", exchange, count);
            }

            // Group by expiration
            let mut expiration_count = std::collections::HashMap::new();
            for option in &options {
                *expiration_count
                    .entry(option.expiration_date.clone())
                    .or_insert(0) += 1;
            }

            debug!("   📅 Top expirations by option count:");
            let mut sorted_exps: Vec<_> = expiration_count.iter().collect();
            sorted_exps.sort_by(|a, b| b.1.cmp(a.1));

            for (i, (exp_date, count)) in sorted_exps.iter().take(5).enumerate() {
                debug!("      {}. {} - {} options", i + 1, exp_date, count);
            }

            // Test 2: Get specific future option
            info!("\n📊 Test 2: Getting specific future options...");

            // Test with first few options
            for option in options.iter().take(3) {
                match tasty.get_future_option(&option.symbol.0).await {
                    Ok(specific_option) => {
                        info!(
                            "✅ Retrieved specific future option: {}",
                            specific_option.symbol.0
                        );
                        debug!("   📊 Details:");
                        debug!(
                            "      - Underlying: {}",
                            specific_option.underlying_symbol.0
                        );
                        debug!("      - Strike: ${}", specific_option.strike_price);
                        debug!("      - Type: {}", specific_option.option_type);
                        debug!("      - Expiration: {}", specific_option.expiration_date);
                        debug!("      - Maturity Date: {}", specific_option.maturity_date);
                        debug!(
                            "      - Exchange Symbol: {}",
                            specific_option.exchange_symbol
                        );
                        debug!("      - Multiplier: {}", specific_option.multiplier);
                        debug!(
                            "      - Underlying Count: {}",
                            specific_option.underlying_count
                        );
                        debug!("      - Notional Value: {}", specific_option.notional_value);
                        debug!("      - Display Factor: {}", specific_option.display_factor);
                        debug!(
                            "      - Settlement Type: {}",
                            specific_option.settlement_type
                        );
                        debug!("      - Strike Factor: {}", specific_option.strike_factor);
                        debug!("      - Is Confirmed: {}", specific_option.is_confirmed);
                        debug!(
                            "      - Is Exercisable Weekly: {}",
                            specific_option.is_exercisable_weekly
                        );
                        debug!(
                            "      - Last Trade Time: {}",
                            specific_option.last_trade_time
                        );
                        debug!(
                            "      - Stops Trading At: {}",
                            specific_option.stops_trading_at
                        );
                        debug!("      - Expires At: {}", specific_option.expires_at);

                        // Show future option product details
                        debug!("   📊 Future Option Product:");
                        debug!(
                            "      - Root Symbol: {}",
                            specific_option.future_option_product.root_symbol
                        );
                        debug!(
                            "      - Code: {}",
                            specific_option.future_option_product.code
                        );
                        debug!(
                            "      - Cash Settled: {}",
                            specific_option.future_option_product.cash_settled
                        );
                        debug!(
                            "      - Exchange: {}",
                            specific_option.future_option_product.exchange
                        );
                        debug!(
                            "      - Product Type: {}",
                            specific_option.future_option_product.product_type
                        );
                        debug!(
                            "      - Market Sector: {}",
                            specific_option.future_option_product.market_sector
                        );
                    }
                    Err(e) => {
                        let error_msg = format!("{}", e);
                        if error_msg.contains("404") || error_msg.contains("record_not_found") {
                            debug!(
                                "ℹ️ Future option {} not found (404) - may be expired or invalid",
                                option.symbol.0
                            );
                        } else {
                            error!(
                                "❌ Error getting specific future option {}: {}",
                                option.symbol.0, e
                            );
                        }
                    }
                }
            }
        } else {
            info!("   ℹ️ No future options found for specified symbols");
        }
    }

    info!("\n✅ Future options testing completed!");

    Ok(())
}
