/******************************************************************************
    Author: Joaquín Béjar García
    Email: jb@taunais.com 
    Date: 7/3/25
 ******************************************************************************/

use std::env;
use chrono::{Local, NaiveDate};
use rust_decimal::Decimal;
use tastytrade_rs::TastyTrade;
use tastytrade_rs::utils::config::Config;
use tastytrade_rs::utils::logger::setup_logger;
use tastytrade_rs::Symbol;
use tracing::{info, debug};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("TastyTrade MSFT 0DTE Options Example");
    info!("-----------------------------------");

    // Check if environment variables are set
    if env::var("TASTYTRADE_USERNAME").is_err() || env::var("TASTYTRADE_PASSWORD").is_err() {
        info!("Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables.");
        info!("Example:");
        info!("  export TASTYTRADE_USERNAME=your_username");
        info!("  export TASTYTRADE_PASSWORD=your_password");
        info!("  export TASTYTRADE_USE_DEMO=true");
        info!("  export LOGLEVEL=DEBUG");
        std::process::exit(1);
    }

    // Load configuration from environment variables
    let config = Config::from_env();
    info!("Configuration loaded, connecting to {}...", 
        if config.use_demo { "demo environment" } else { "production environment" });

    // Login to the TastyTrade API
    let tasty = TastyTrade::login(&config).await?;
    info!("Successfully logged in!");

    // Today's date for 0DTE identification
    let today = Local::now().date_naive();
    info!("Looking for MSFT options expiring today ({})", today.format("%Y-%m-%d"));

    // Symbol for Microsoft
    let msft_symbol = Symbol("MSFT".to_string());

    // Get option chain for MSFT
    match tasty.nested_option_chain_for(msft_symbol.clone()).await {
        Ok(chain) => {
            info!("Successfully retrieved option chain for MSFT");
            info!("Root symbol: {}, Shares per contract: {}", chain.root_symbol.0, chain.shares_per_contract);
            info!("Total available expirations: {}", chain.expirations.len());

            // Find today's expiration (0DTE)
            let mut found_0dte = false;

            for expiration in &chain.expirations {
                // Parse the expiration date from the string
                let exp_date = match NaiveDate::parse_from_str(&expiration.expiration_date, "%Y-%m-%d") {
                    Ok(date) => date,
                    Err(_) => {
                        debug!("Could not parse expiration date: {}", expiration.expiration_date);
                        continue;
                    }
                };

                // Check if this expiration is today (0DTE)
                if exp_date == today {
                    found_0dte = true;
                    info!("===== FOUND 0DTE OPTIONS FOR MSFT =====");
                    info!("Expiration date: {}", expiration.expiration_date);
                    info!("Days to expiration: {} (should be 0)", expiration.days_to_expiration);
                    info!("Settlement type: {}", expiration.settlement_type);
                    info!("Number of available strikes: {}", expiration.strikes.len());

                    // Check if we have any strikes available
                    if expiration.strikes.is_empty() {
                        info!("No strikes available for this expiration.");
                        continue;
                    }

                    // Get current price of MSFT using the API
                    let current_price = match tasty.get_equity_info(&msft_symbol).await {
                        Ok(_) => {
                            // In a real implementation, you would get the current price from a quote
                            // For simplicity, we'll get a rough estimate from the strikes
                            let middle_index = expiration.strikes.len() / 2;
                            expiration.strikes[middle_index].strike_price
                        },
                        Err(e) => {
                            info!("Could not get current MSFT price: {}", e);
                            continue;
                        }
                    };

                    info!("Estimated current price: {}", current_price);

                    // Group strikes around current price
                    let mut near_the_money_strikes = expiration.strikes.iter()
                        .filter(|strike| {
                            let diff = (strike.strike_price - current_price).abs();
                            // Show strikes within approximately 5% of current price
                            diff / current_price * Decimal::ONE_HUNDRED < 5.into()
                        })
                        .collect::<Vec<_>>();

                    // Sort by strike price
                    near_the_money_strikes.sort_by(|a, b| a.strike_price.partial_cmp(&b.strike_price).unwrap());

                    info!("Near-the-money 0DTE options for MSFT:");
                    for (i, strike) in near_the_money_strikes.iter().enumerate() {
                        info!("{}. Strike price: ${}", i + 1, strike.strike_price);
                        info!("   Call symbol: {}", strike.call.0);
                        info!("   Put symbol: {}", strike.put.0);

                        // Get option info for the call and put if desired
                        debug!("   To get detailed quotes or to trade these options,");
                        debug!("   use the symbols above with quote_streamer or order methods");
                    }

                    // Print information about all available strikes
                    info!("All available 0DTE strikes:");
                    info!("Lowest strike: ${}", expiration.strikes.first().unwrap().strike_price);
                    info!("Highest strike: ${}", expiration.strikes.last().unwrap().strike_price);
                    info!("Number of strike prices: {}", expiration.strikes.len());

                    // Provide a simple histogram of strikes
                    let strike_range = expiration.strikes.last().unwrap().strike_price -
                        expiration.strikes.first().unwrap().strike_price;
                    let bucket_size = strike_range / Decimal::TEN;

                    if bucket_size > 0.into() {
                        info!("Distribution of strikes (rough histogram):");
                        let mut current_bucket = expiration.strikes.first().unwrap().strike_price;
                        let mut i = 0;

                        while current_bucket <= expiration.strikes.last().unwrap().strike_price {
                            let next_bucket = current_bucket + bucket_size;
                            let count = expiration.strikes.iter()
                                .filter(|s| s.strike_price >= current_bucket && s.strike_price < next_bucket)
                                .count();

                            info!("${} to ${}: {} strikes", 
                                 current_bucket, next_bucket, count);

                            current_bucket = next_bucket;
                            i += 1;
                            if i > 15 { // Prevent infinite loops
                                break;
                            }
                        }
                    }

                    // Break because we found what we were looking for
                    break;
                }
            }

            if !found_0dte {
                info!("No 0DTE options found for MSFT today.");
                info!("This could mean that:");
                info!("1. There are no options expiring today for MSFT");
                info!("2. Today might not be a trading day (weekend or holiday)");
                info!("3. 0DTE options might not be available for this symbol");

                // Show the closest expiration instead
                if let Some(exp) = &chain.expirations.iter().min_by_key(|e| e.days_to_expiration) {
                    info!("The closest available expiration is: {}", exp.expiration_date);
                    info!("Days to expiration: {}", exp.days_to_expiration);
                    info!("Number of available strikes: {}", exp.strikes.len());
                }
            }
        },
        Err(e) => {
            info!("Error retrieving MSFT option chain: {}", e);
            info!("Please check if the symbol is correct and if your account has access to options data.");
        }
    }

    info!("MSFT 0DTE options example completed!");
    Ok(())
}