/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 7/3/25
******************************************************************************/

use std::env;
use tastytrade::prelude::*;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("TastyTrade Available Symbols Example");
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
    let config = TastyTradeConfig::from_env();
    info!(
        "Configuration loaded, connecting to {}...",
        if config.use_demo {
            "demo environment"
        } else {
            "production environment"
        }
    );

    // Login to the TastyTrade API
    let tasty = TastyTrade::login(&config).await?;
    info!("Successfully logged in!");

    // Get account information
    let accounts = tasty.accounts().await?;
    if accounts.is_empty() {
        info!("No accounts found. Please make sure your account is properly set up.");
        return Ok(());
    }

    info!("Found {} account(s)", accounts.len());
    let account = &accounts[0]; // Use the first account
    info!("Using account: {}", account.number().0);

    // Get positions to see which symbols are available in the account
    let positions = account.positions().await?;
    info!("Current positions: {}", positions.len());

    if !positions.is_empty() {
        info!("Symbols in your positions:");
        for (i, position) in positions.iter().enumerate() {
            info!(
                "  {}. Symbol: {}, Type: {:?}, Underlying: {}",
                i + 1,
                position.symbol.0,
                position.instrument_type,
                position.underlying_symbol.0
            );

            // For stock positions, get option chain information
            if let tastytrade::InstrumentType::Equity = position.instrument_type {
                debug!("Getting option chain for {}", position.symbol.0);
                match tasty
                    .nested_option_chain_for(position.symbol.as_symbol())
                    .await
                {
                    Ok(chain) => {
                        info!(
                            "    Available option expirations for {}:",
                            position.symbol.0
                        );
                        for (j, exp) in chain.expirations.iter().enumerate().take(5) {
                            info!(
                                "      {}. Expiration: {}, Days to expiry: {}, Strike count: {}",
                                j + 1,
                                exp.expiration_date,
                                exp.days_to_expiration,
                                exp.strikes.len()
                            );

                            // Show a few strikes as example
                            if !exp.strikes.is_empty() {
                                info!("        Example strikes:");
                                for (k, strike) in exp.strikes.iter().enumerate().take(3) {
                                    info!(
                                        "          {}. Price: {}, Call: {}, Put: {}",
                                        k + 1,
                                        strike.strike_price,
                                        strike.call.0,
                                        strike.put.0
                                    );
                                }
                                if exp.strikes.len() > 3 {
                                    info!(
                                        "          ... and {} more strikes",
                                        exp.strikes.len() - 3
                                    );
                                }
                            }
                        }
                        if chain.expirations.len() > 5 {
                            info!(
                                "      ... and {} more expirations",
                                chain.expirations.len() - 5
                            );
                        }
                    }
                    Err(e) => {
                        info!(
                            "    Could not retrieve option chain for {}: {}",
                            position.symbol.0, e
                        );
                    }
                }
            }
        }
    } else {
        info!("No positions found in your account.");
    }

    // List some popular equity symbols as examples
    info!("Querying information for some popular symbols:");
    let popular_symbols = vec!["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"];

    for symbol in popular_symbols {
        let symbol = Symbol(symbol.to_string());
        info!("Information for symbol: {}", symbol.0);

        // Try to get equity info for the symbol
        match tasty.get_equity_info(&symbol).await {
            Ok(info) => {
                info!(
                    "  Equity symbol: {}, Streamer symbol: {}",
                    info.symbol.0, info.streamer_symbol.0
                );

                // Try to get option chain for the symbol
                match tasty.nested_option_chain_for(symbol.as_symbol()).await {
                    Ok(chain) => {
                        info!(
                            "  Root symbol: {}, Shares per contract: {}",
                            chain.root_symbol.0, chain.shares_per_contract
                        );
                        info!("  Total expirations available: {}", chain.expirations.len());

                        // Show first few expirations
                        if !chain.expirations.is_empty() {
                            info!("  First few expirations:");
                            for (i, exp) in chain.expirations.iter().enumerate().take(3) {
                                info!(
                                    "    {}. {}, {} days to expiry, {} strikes",
                                    i + 1,
                                    exp.expiration_date,
                                    exp.days_to_expiration,
                                    exp.strikes.len()
                                );
                            }
                            if chain.expirations.len() > 3 {
                                info!("    ... and {} more", chain.expirations.len() - 3);
                            }
                        }
                    }
                    Err(e) => {
                        info!("  Could not retrieve option chain: {}", e);
                    }
                }
            }
            Err(e) => {
                info!("  Could not retrieve equity info: {}", e);
            }
        }

        info!("");
    }

    // Attempt to get a list of watchlists if available
    // Note: This would require additional API methods not visible in the provided code
    // This is a placeholder for potential future functionality
    info!("Note: To get a comprehensive list of all available symbols,");
    info!("you might want to use TastyTrade's official watchlists or symbol search functionality.");
    info!("This example shows how to access symbols you already have positions in");
    info!("and how to get information about specific popular symbols.");

    info!("Available symbols example completed successfully!");
    Ok(())
}
