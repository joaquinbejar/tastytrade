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

    println!("🚀 Testing list_active_equities method");
    println!("======================================");

    // Load configuration from environment
    let config = TastyTradeConfig::from_env();

    // Check if we have valid credentials
    if !config.has_valid_credentials() {
        eprintln!(
            "❌ No valid credentials found. Please set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN environment variables."
        );
        return Err("Missing credentials".into());
    }

    println!("🔐 Logging into TastyTrade...");
    let tasty = TastyTrade::connect(&config).await?;
    println!("✅ Successfully logged in!");

    println!();
    println!("📊 Testing list_active_equities with different page offsets...");

    // Test all available pages
    let mut page = 0;
    loop {
        println!();
        println!("📄 ==================== PAGE {} ====================", page);

        match tasty
            .list_active_equities(
                &ActiveEquityFilter::new().with_page(PageRequest::new().with_page_offset(page)),
            )
            .await
        {
            Ok(paginated_result) => {
                let items_count = paginated_result.items.len();
                let pagination = &paginated_result.pagination;

                println!("📊 Page {}: {} items found", page, items_count);
                println!("📊 Pagination info:");
                println!("   - page_offset: {}", pagination.page_offset);
                println!("   - total_pages: {}", pagination.total_pages);
                println!("   - total_items: {}", pagination.total_items);
                println!("   - per_page: {}", pagination.per_page);
                println!("   - item_offset: {}", pagination.item_offset);
                println!("   - current_item_count: {}", pagination.current_item_count);

                if items_count > 0 {
                    println!("📋 EQUITY INSTRUMENTS FOUND:");
                    for (i, equity) in paginated_result.items.iter().enumerate() {
                        println!(
                            "   {}. Symbol: {} | ID: {} | Active: {} | Description: {} | Instrument Type: {}",
                            i + 1,
                            equity.symbol.0,
                            equity.id,
                            equity.active,
                            equity.description,
                            equity.instrument_type
                        );

                        // Show additional details for first few items
                        if i < 3 {
                            println!("      - Type: {:?}", equity.instrument_type);
                            println!("      - Market: {}", equity.listed_market);
                            println!("      - CUSIP: {:?}", equity.cusip);
                            println!("      - Is Index: {}", equity.is_index);
                            println!("      - Is ETF: {}", equity.is_etf);
                        }
                    }
                } else {
                    println!("❌ NO ITEMS FOUND ON PAGE {}", page);
                    if pagination.total_items > 0 {
                        println!(
                            "⚠️  But API indicates {} total items exist!",
                            pagination.total_items
                        );
                    }
                }

                // Stop if we've gone past the total pages
                if !paginated_result.has_more() {
                    println!(
                        "🛑 Reached end of pages (page {} is the last page of {})",
                        page, pagination.total_pages
                    );
                    break;
                }

                // Stop if this page is empty and we have no total items
                if items_count == 0 && pagination.total_items == 0 {
                    println!("🛑 No more data available");
                    break;
                }

                // Move to next page
                page += 1;
            }
            Err(e) => {
                eprintln!("❌ Error fetching page {}: {}", page, e);
                break;
            }
        }
    }

    println!();
    println!("✅ Test completed!");

    Ok(())
}
