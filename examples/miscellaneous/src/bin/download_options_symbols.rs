/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("🚀 TastyTrade Options Symbol Downloader");
    info!("=======================================");

    // Download all symbols
    let symbols = download_options_symbols().await?;

    // Display summary
    let equity_options = symbols
        .iter()
        .filter(|s| matches!(s.instrument_type, InstrumentType::EquityOption))
        .count();
    let future_options = symbols
        .iter()
        .filter(|s| matches!(s.instrument_type, InstrumentType::FutureOption))
        .count();

    println!();
    info!("📊 Summary:");
    info!("  • EquityOptions: {}", equity_options);
    info!("  • FutureOptions: {}", future_options);
    info!("  • Total: {}", symbols.len());

    // Save to file
    save_symbols_to_file(&symbols, "tastytrade_options_symbols.json").await?;

    // Display first few symbols as examples
    println!();
    info!("🔍 Sample symbols:");
    for (i, symbol) in symbols.iter().take(5).enumerate() {
        info!(
            "  {}. {} - {} ({})",
            i + 1,
            symbol.symbol,
            symbol.name,
            if matches!(symbol.instrument_type, InstrumentType::EquityOption) {
                "Equity Option"
            } else if matches!(symbol.instrument_type, InstrumentType::FutureOption) {
                "Future Option"
            } else {
                "Other"
            }
        );
    }

    if symbols.len() > 5 {
        info!("  ... and {} more", symbols.len() - 5);
    }

    println!();
    info!("✅ Download completed successfully!");

    Ok(())
}
