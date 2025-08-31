/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 7/3/25
******************************************************************************/

// examples/get_msft_price.rs

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::env;
use std::time::Duration;
use tastytrade::prelude::*;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    // Load configuration from environment variables
    let config = TastyTradeConfig::new();
    info!(
        "Configuration loaded, connecting to {}...",
        if config.use_demo {
            "demo environment"
        } else {
            "production environment"
        }
    );

    info!("TastyTrade MSFT Price Example (Updated API)");
    info!("-----------------------------------------");

    // Check if environment variables are set
    if env::var("TASTYTRADE_USERNAME").is_err() || env::var("TASTYTRADE_PASSWORD").is_err() {
        info!("Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables.");
        std::process::exit(1);
    }

    // Login to the TastyTrade API
    let tasty = TastyTrade::login(&config).await?;
    info!("Successfully logged in!");

    // Define the symbol
    let symbol = Symbol("MSFT".to_string());

    // Try to get tokens using the new endpoint
    debug!("Requesting quote streamer tokens using new endpoint");
    let tokens = tasty.quote_streamer_tokens().await?;
    debug!(
        "Received tokens: streamer_url={}, level={}",
        tokens.streamer_url, tokens.level
    );

    // Create quote streamer
    debug!("Creating quote streamer");
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    debug!("Quote streamer created successfully");

    // Create subscription
    debug!("Creating subscription with flags: {}", DXF_ET_QUOTE);
    let quote_sub = &mut *quote_streamer.create_sub(DXF_ET_QUOTE);
    debug!("Subscription created successfully");

    // Get streamer symbol
    debug!(
        "Getting streamer symbol for {} with type {:?}",
        symbol.0,
        InstrumentType::Equity
    );
    let streamer_symbol = tasty
        .get_streamer_symbol(&InstrumentType::Equity, &symbol)
        .await?;
    debug!("Streamer symbol obtained: {}", streamer_symbol.0);

    // Add symbol to subscription
    debug!("Adding symbol to subscription");
    quote_sub.add_symbols(&[streamer_symbol.clone()]);
    debug!("Symbol added to subscription");

    // Wait for a quote
    info!("Waiting for quote data for {}...", symbol.0);
    info!("Will wait up to 30 seconds for a response");

    let mut current_price: Option<Decimal> = None;
    let timeout = tokio::time::Instant::now() + Duration::from_secs(30);

    while current_price.is_none() && tokio::time::Instant::now() < timeout {
        debug!("Waiting for quote event...");

        match tokio::time::timeout(Duration::from_secs(1), quote_sub.get_event()).await {
            Ok(Ok(Event { sym, data })) => {
                debug!("Received event for symbol: {}", sym);
                if let EventData::Quote(quote) = data {
                    // Use mid price
                    let mid_price = (quote.bid_price + quote.ask_price) / 2.0;
                    current_price = Some(Decimal::from_f64(mid_price).unwrap_or_default());
                    info!(
                        "Current price for {}: ${}",
                        symbol.0,
                        current_price.unwrap()
                    );
                    break;
                } else {
                    debug!("Received non-quote event: {:?}", data);
                }
            }
            Ok(Err(e)) => {
                error!("Error getting event: {:?}", e);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(_) => {
                debug!("Timeout waiting for event, retrying...");
            }
        }
    }

    if let Some(price) = current_price {
        info!("Successfully obtained price for {}: ${}", symbol.0, price);
    } else {
        error!(
            "Could not get current price for {} after 30 seconds.",
            symbol.0
        );
    }

    info!("Example completed");
    Ok(())
}
