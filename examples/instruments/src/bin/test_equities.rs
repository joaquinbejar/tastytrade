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

    info!("🚀 Testing equities endpoints");
    info!("==============================");

    // Load configuration from environment
    let config = TastyTradeConfig::from_env();

    // Check if we have valid credentials
    if !config.has_valid_credentials() {
        error!("❌ No valid credentials found. Please set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN environment variables.");
        return Err("Missing credentials".into());
    }

    info!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::connect(&config).await?;
    info!("✅ Successfully logged in!");

    // Test 1: List active equities with pagination
    info!("\n📊 Test 1: Listing active equities (paginated)...");
    match tasty
        .list_active_equities(&ActiveEquityFilter::new().with_page(PageRequest::first()))
        .await
    {
        Ok(paginated_equities) => {
            info!(
                "✅ Retrieved {} active equities from page 1",
                paginated_equities.items.len()
            );
            debug!("   📊 Pagination info:");
            debug!(
                "      - Current page: {}",
                paginated_equities.pagination.page_offset
            );
            debug!(
                "      - Items per page: {}",
                paginated_equities.pagination.per_page
            );
            debug!(
                "      - Total pages: {}",
                paginated_equities.pagination.total_pages
            );
            debug!(
                "      - Total items: {}",
                paginated_equities.pagination.total_items
            );

            if !paginated_equities.items.is_empty() {
                // Show first few equities
                for (i, equity) in paginated_equities.items.iter().enumerate().take(5) {
                    debug!(
                        "   {}. {} | {} | Market: {} | ETF: {} | Index: {}",
                        i + 1,
                        equity.symbol.0,
                        equity.short_description,
                        equity.listed_market,
                        equity.is_etf,
                        equity.is_index
                    );

                    if i < 2 {
                        debug!("      - ID: {}", equity.id);
                        debug!("      - CUSIP: {:?}", equity.cusip);
                        debug!("      - Active: {}", equity.active);
                        debug!("      - Closing Only: {}", equity.is_closing_only);
                        debug!(
                            "      - Options Closing Only: {}",
                            equity.is_options_closing_only
                        );
                        debug!(
                            "      - Fractional Eligible: {}",
                            equity.is_fractional_quantity_eligible
                        );
                    }
                }

                if paginated_equities.items.len() > 5 {
                    debug!(
                        "   ... and {} more equities",
                        paginated_equities.items.len() - 5
                    );
                }

                // Analyze equity types
                let etfs = paginated_equities.items.iter().filter(|e| e.is_etf).count();
                let indices = paginated_equities
                    .items
                    .iter()
                    .filter(|e| e.is_index)
                    .count();
                let fractional_eligible = paginated_equities
                    .items
                    .iter()
                    .filter(|e| e.is_fractional_quantity_eligible)
                    .count();
                let closing_only = paginated_equities
                    .items
                    .iter()
                    .filter(|e| e.is_closing_only)
                    .count();

                info!("   📈 Analysis of page 1:");
                debug!("      - ETFs: {}", etfs);
                debug!("      - Indices: {}", indices);
                debug!("      - Fractional Eligible: {}", fractional_eligible);
                debug!("      - Closing Only: {}", closing_only);

                // Test 2: Get specific equities by symbols
                info!("\n📊 Test 2: Getting specific equities by symbols...");

                let test_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "SPY"];

                match tasty
                    .list_equities(&EquityFilter::for_symbols(&test_symbols))
                    .await
                {
                    Ok(specific_equities) => {
                        info!(
                            "✅ Retrieved {} equities for specific symbols",
                            specific_equities.len()
                        );

                        for equity in &specific_equities {
                            debug!("   📊 {}: {}", equity.symbol.0, equity.short_description);
                            debug!(
                                "      - Market: {} | ETF: {} | Active: {}",
                                equity.listed_market, equity.is_etf, equity.active
                            );
                        }
                    }
                    Err(e) => {
                        error!("❌ Error getting specific equities: {}", e);
                    }
                }

                // Test 3: Get individual equity details
                info!("\n📊 Test 3: Getting individual equity details...");

                for symbol in test_symbols.iter().take(3) {
                    // Test get_equity method
                    match tasty.get_equity(symbol).await {
                        Ok(equity) => {
                            info!("✅ Retrieved equity details for {}", equity.symbol.0);
                            debug!("   📊 Full details:");
                            debug!("      - Description: {}", equity.description);
                            debug!("      - Listed Market: {}", equity.listed_market);
                            debug!(
                                "      - Market Time Collection: {}",
                                equity.market_time_instrument_collection
                            );
                            if let Some(lendability) = &equity.lendability {
                                debug!("      - Lendability: {}", lendability);
                            }
                            if let Some(borrow_rate) = &equity.borrow_rate {
                                debug!("      - Borrow Rate: {}", borrow_rate);
                            }
                        }
                        Err(e) => {
                            error!("❌ Error getting equity details for {}: {}", symbol, e);
                        }
                    }

                    // Test get_equity_info method
                    match tasty.get_equity_info(symbol).await {
                        Ok(equity_info) => {
                            info!("✅ Retrieved equity info for {}", equity_info.symbol.0);
                            debug!("   📊 Info details:");
                            debug!("      - Streamer Symbol: {}", equity_info.streamer_symbol.0);
                        }
                        Err(e) => {
                            error!("❌ Error getting equity info for {}: {}", symbol, e);
                        }
                    }
                }

                // Test 4: Analyze markets
                info!("\n📊 Test 4: Analyzing markets...");
                let mut market_count = std::collections::HashMap::new();

                for equity in &paginated_equities.items {
                    *market_count
                        .entry(equity.listed_market.clone())
                        .or_insert(0) += 1;
                }

                info!(
                    "✅ Found equities across {} different markets",
                    market_count.len()
                );

                debug!("   📊 Markets by equity count:");
                let mut sorted_markets: Vec<_> = market_count.iter().collect();
                sorted_markets.sort_by(|a, b| b.1.cmp(a.1));

                for (i, (market, count)) in sorted_markets.iter().enumerate().take(5) {
                    debug!("      {}. {}: {} equities", i + 1, market, count);
                }
            } else {
                info!("   ℹ️ No active equities found on page 1");
            }
        }
        Err(e) => {
            error!("❌ Error listing active equities: {}", e);
        }
    }

    info!("\n✅ Equities testing completed!");

    Ok(())
}
