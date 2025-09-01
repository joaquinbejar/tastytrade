/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 1/9/25
******************************************************************************/

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    info!("🚀 Testing quantity decimal precisions endpoints");
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

    // Test: List all quantity decimal precisions
    info!("\n📊 Test: Listing all quantity decimal precisions...");

    match tasty.list_quantity_decimal_precisions().await {
        Ok(precisions) => {
            info!("✅ Found {} quantity decimal precisions", precisions.len());

            if !precisions.is_empty() {
                // Show all precisions (usually not too many)
                for (i, precision) in precisions.iter().enumerate() {
                    info!(
                        "   {}. Instrument Type: {:?} | Value: {} | Min Increment Precision: {}",
                        i + 1,
                        precision.instrument_type,
                        precision.value,
                        precision.minimum_increment_precision
                    );

                    if let Some(symbol) = &precision.symbol {
                        info!("      - Symbol: {}", symbol.0);
                    } else {
                        info!("      - Symbol: None (applies to all symbols of this type)");
                    }
                }

                // Analyze by instrument type
                let mut type_count = std::collections::HashMap::new();
                let mut type_values = std::collections::HashMap::new();

                for precision in &precisions {
                    let type_key = format!("{:?}", precision.instrument_type);
                    *type_count.entry(type_key.clone()).or_insert(0) += 1;

                    type_values
                        .entry(type_key)
                        .or_insert_with(Vec::new)
                        .push(precision.value);
                }

                info!("   📈 Analysis by instrument type:");
                for (inst_type, count) in type_count.iter() {
                    info!("      - {}: {} precision rules", inst_type, count);

                    if let Some(values) = type_values.get(inst_type) {
                        let unique_values: std::collections::HashSet<_> = values.iter().collect();
                        info!(
                            "        Values: {:?}",
                            unique_values.iter().collect::<Vec<_>>()
                        );
                    }
                }

                // Analyze by precision value
                let mut value_count = std::collections::HashMap::new();
                for precision in &precisions {
                    *value_count.entry(precision.value).or_insert(0) += 1;
                }

                info!("   📊 Precision values distribution:");
                let mut sorted_values: Vec<_> = value_count.iter().collect();
                sorted_values.sort_by(|a, b| a.0.cmp(b.0));

                for (value, count) in sorted_values.iter() {
                    info!(
                        "      - {} decimal places: {} instrument types",
                        value, count
                    );
                }

                // Analyze minimum increment precision
                let mut min_increment_count = std::collections::HashMap::new();
                for precision in &precisions {
                    *min_increment_count
                        .entry(precision.minimum_increment_precision)
                        .or_insert(0) += 1;
                }

                info!("   📊 Minimum increment precision distribution:");
                let mut sorted_min_increments: Vec<_> = min_increment_count.iter().collect();
                sorted_min_increments.sort_by(|a, b| a.0.cmp(b.0));

                for (min_increment, count) in sorted_min_increments.iter() {
                    info!(
                        "      - {} minimum increment precision: {} instrument types",
                        min_increment, count
                    );
                }

                // Find symbol-specific precisions
                let symbol_specific: Vec<_> =
                    precisions.iter().filter(|p| p.symbol.is_some()).collect();

                let general_rules: Vec<_> =
                    precisions.iter().filter(|p| p.symbol.is_none()).collect();

                info!("   📊 Rule specificity:");
                info!(
                    "      - General rules (no specific symbol): {}",
                    general_rules.len()
                );
                info!("      - Symbol-specific rules: {}", symbol_specific.len());

                if !symbol_specific.is_empty() {
                    info!("   📊 Symbol-specific precision rules:");
                    for precision in symbol_specific.iter().take(5) {
                        if let Some(symbol) = &precision.symbol {
                            info!(
                                "      - {}: {} decimal places (type: {:?})",
                                symbol.0, precision.value, precision.instrument_type
                            );
                        }
                    }

                    if symbol_specific.len() > 5 {
                        info!(
                            "      ... and {} more symbol-specific rules",
                            symbol_specific.len() - 5
                        );
                    }
                }

                // Show practical examples
                info!("   📊 Practical examples:");
                for precision in precisions.iter().take(3) {
                    let symbol_info = if let Some(symbol) = &precision.symbol {
                        format!("for symbol {}", symbol.0)
                    } else {
                        "for all symbols of this type".to_string()
                    };

                    info!(
                        "      - {:?} {} can have up to {} decimal places",
                        precision.instrument_type, symbol_info, precision.value
                    );
                    info!(
                        "        Minimum increment precision: {} decimal places",
                        precision.minimum_increment_precision
                    );
                }
            } else {
                info!("   ℹ️ No quantity decimal precisions found");
            }
        }
        Err(e) => {
            error!("❌ Error listing quantity decimal precisions: {}", e);
        }
    }

    info!("\n✅ Quantity decimal precisions testing completed!");

    Ok(())
}
