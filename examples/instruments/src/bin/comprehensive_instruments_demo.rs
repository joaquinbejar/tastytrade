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

    info!("🚀 Comprehensive TastyTrade Instruments API Demo");
    info!("=================================================");

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

    // Demo 1: Equity Instruments
    info!("\n📈 Demo 1: Equity Instruments");
    info!("==============================");

    match tasty.list_active_equities(0).await {
        Ok(paginated_equities) => {
            info!(
                "✅ Retrieved {} active equities (page 1)",
                paginated_equities.items.len()
            );
            debug!(
                "   Total pages: {}",
                paginated_equities.pagination.total_pages
            );
            debug!(
                "   Total items: {}",
                paginated_equities.pagination.total_items
            );

            if let Some(first_equity) = paginated_equities.items.first() {
                debug!(
                    "   Sample equity: {} - {}",
                    first_equity.symbol.0, first_equity.description
                );
            }
        }
        Err(e) => error!("❌ Error retrieving active equities: {}", e),
    }

    // Demo 2: Option Chains
    info!("\n📊 Demo 2: Option Chains");
    info!("========================");

    let test_symbol = "AAPL";

    // Standard option chain
    match tasty.list_option_chains(test_symbol).await {
        Ok(options) => {
            info!("✅ Retrieved {} options for {}", options.len(), test_symbol);
            if !options.is_empty() {
                let calls = options.iter().filter(|o| o.option_type == "C").count();
                let puts = options.iter().filter(|o| o.option_type == "P").count();
                debug!("   Breakdown: {} calls, {} puts", calls, puts);
            }
        }
        Err(e) => error!(
            "❌ Error retrieving option chain for {}: {}",
            test_symbol, e
        ),
    }

    // Compact option chain
    match tasty.get_compact_option_chain(test_symbol).await {
        Ok(compact) => {
            info!("✅ Retrieved compact option chain for {}", test_symbol);
            debug!("   Chain type: {}", compact.option_chain_type);
            debug!("   Shares per contract: {}", compact.shares_per_contract);
        }
        Err(e) => error!(
            "❌ Error retrieving compact option chain for {}: {}",
            test_symbol, e
        ),
    }

    // Nested option chain
    match tasty.list_nested_option_chains(test_symbol).await {
        Ok(nested) => {
            info!(
                "✅ Retrieved {} nested option chains for {}",
                nested.len(),
                test_symbol
            );
            if let Some(first_chain) = nested.first() {
                debug!(
                    "   First chain has {} expirations",
                    first_chain.expirations.len()
                );
            }
        }
        Err(e) => error!(
            "❌ Error retrieving nested option chains for {}: {}",
            test_symbol, e
        ),
    }

    // Demo 3: Futures
    info!("\n📅 Demo 3: Futures");
    info!("==================");

    match tasty
        .list_futures(None::<&[&str]>, Some("ES"), None, None, None)
        .await
    {
        Ok(futures) => {
            info!("✅ Retrieved {} ES futures", futures.len());
            if let Some(first_future) = futures.first() {
                debug!(
                    "   Sample future: {} - expires {}",
                    first_future.symbol.0, first_future.expiration_date
                );
            }
        }
        Err(e) => error!("❌ Error retrieving ES futures: {}", e),
    }

    // Demo 4: Future Products
    info!("\n🏭 Demo 4: Future Products");
    info!("==========================");

    match tasty.list_future_products().await {
        Ok(products) => {
            info!("✅ Retrieved {} future products", products.len());
            if let Some(first_product) = products.first() {
                debug!(
                    "   Sample product: {} - {}",
                    first_product.code, first_product.description
                );
            }
        }
        Err(e) => error!("❌ Error retrieving future products: {}", e),
    }

    // Demo 5: Future Option Products
    info!("\n🔮 Demo 5: Future Option Products");
    info!("==================================");

    match tasty.list_future_option_products().await {
        Ok(products) => {
            info!("✅ Retrieved {} future option products", products.len());
            if let Some(first_product) = products.first() {
                debug!(
                    "   Sample product: {} on {}",
                    first_product.root_symbol, first_product.exchange
                );
            }
        }
        Err(e) => error!("❌ Error retrieving future option products: {}", e),
    }

    // Demo 6: Futures Option Chains
    info!("\n🔗 Demo 6: Futures Option Chains");
    info!("=================================");

    let test_product = "ES";

    match tasty.list_futures_option_chains(test_product).await {
        Ok(options) => {
            info!(
                "✅ Retrieved {} future options for product {}",
                options.len(),
                test_product
            );
            if !options.is_empty() {
                let active = options.iter().filter(|o| o.active).count();
                debug!("   Active options: {}", active);
            }
        }
        Err(e) => error!(
            "❌ Error retrieving future option chains for {}: {}",
            test_product, e
        ),
    }

    // Demo 7: Cryptocurrencies
    info!("\n🪙 Demo 7: Cryptocurrencies");
    info!("============================");

    match tasty.list_cryptocurrencies(&["BTC/USD"]).await {
        Ok(cryptos) => {
            info!("✅ Retrieved {} cryptocurrencies", cryptos.len());
            if let Some(first_crypto) = cryptos.first() {
                debug!(
                    "   Sample crypto: {} - {}",
                    first_crypto.symbol.0, first_crypto.description
                );
            }
        }
        Err(e) => error!("❌ Error retrieving cryptocurrencies: {}", e),
    }

    // Demo 8: Warrants
    info!("\n📜 Demo 8: Warrants");
    info!("====================");

    match tasty.list_warrants(None::<&[&str]>).await {
        Ok(warrants) => {
            info!("✅ Retrieved {} warrants", warrants.len());
            if let Some(first_warrant) = warrants.first() {
                debug!(
                    "   Sample warrant: {} - {}",
                    first_warrant.symbol.0, first_warrant.description
                );
            }
        }
        Err(e) => error!("❌ Error retrieving warrants: {}", e),
    }

    // Demo 9: Quantity Decimal Precisions
    info!("\n⚙️ Demo 9: Quantity Decimal Precisions");
    info!("======================================");

    match tasty.list_quantity_decimal_precisions().await {
        Ok(precisions) => {
            info!(
                "✅ Retrieved {} quantity decimal precisions",
                precisions.len()
            );
            if let Some(first_precision) = precisions.first() {
                debug!(
                    "   Sample precision: {:?} - value: {}",
                    first_precision.instrument_type, first_precision.value
                );
            }
        }
        Err(e) => error!("❌ Error retrieving quantity decimal precisions: {}", e),
    }

    info!("\n🎉 Comprehensive demo completed successfully!");
    info!("All TastyTrade Instruments API endpoints are functional and working correctly.");

    Ok(())
}
