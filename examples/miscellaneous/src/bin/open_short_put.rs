/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 7/3/25
******************************************************************************/

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::env;
use std::time::Duration;
use tastytrade::dxfeed::{self, Event, EventData};
use tastytrade::utils::config::Config;
use tastytrade::utils::logger::setup_logger;
use tastytrade::{
    Action, InstrumentType, OrderBuilder, OrderLegBuilder, OrderType, PriceEffect, Symbol,
    TastyTrade, TimeInForce,
};
use tracing::{error, info, warn};

// Configuration constants
const UNDERLYING_SYMBOL: &str = "MSFT"; // Change to your desired underlying
const CONTRACT_QUANTITY: Decimal = Decimal::ONE; // Number of contracts to sell
const DTE_TARGET: u64 = 7; // Target days to expiration

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("TastyTrade Short ATM Put Example");
    info!("--------------------------------");

    // Check if environment variables are set
    if env::var("TASTYTRADE_USERNAME").is_err() || env::var("TASTYTRADE_PASSWORD").is_err() {
        info!("Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables.");
        info!("Example:");
        info!("  export TASTYTRADE_USERNAME=your_username");
        info!("  export TASTYTRADE_PASSWORD=your_password");
        info!("  export TASTYTRADE_USE_DEMO=true"); // Recommended for testing!
        info!("  export LOGLEVEL=DEBUG");
        std::process::exit(1);
    }

    // Load configuration from environment variables
    let config = Config::from_env();

    // SAFETY WARNING
    if !config.use_demo {
        warn!("!!! WARNING: You are about to execute a real order on a production account !!!");
        warn!(
            "This example will attempt to sell a put option that will obligate you to buy shares."
        );
        warn!("This is NOT paper trading and will use REAL MONEY if not in demo mode.");
        warn!("Press Ctrl+C within 10 seconds to cancel.");

        // Wait 10 seconds to give the user a chance to cancel
        for i in (1..=10).rev() {
            info!("Continuing in {} seconds...", i);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    info!(
        "Connecting to {}...",
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
        error!("No accounts found. Please make sure your account is properly set up.");
        return Ok(());
    }

    let account = &accounts[0]; // Use the first account
    info!("Using account: {}", account.number().0);

    // Check account balance and buying power
    let balance = account.balance().await?;
    info!("Account balance:");
    info!("  Cash balance: ${}", balance.cash_balance);
    info!("  Buying power: ${}", balance.equity_buying_power);

    // Create Symbol object
    let symbol = Symbol(UNDERLYING_SYMBOL.to_string());

    // Step 1: Get current price of the underlying
    info!("Getting current price for {}...", symbol.0);

    // Create a quote streamer to get real-time prices
    info!("Creating quote streamer...");
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    info!("Quote streamer created successfully");

    info!("Creating subscription with flags: {}", dxfeed::DXF_ET_QUOTE);
    let quote_sub = quote_streamer.create_sub(dxfeed::DXF_ET_QUOTE);
    info!("Subscription created successfully");

    info!(
        "Getting streamer symbol for {} with type {:?}...",
        symbol.0,
        InstrumentType::Equity
    );
    let streamer_symbol = tasty
        .get_streamer_symbol(&InstrumentType::Equity, &symbol)
        .await?;
    info!("Streamer symbol obtained: {}", streamer_symbol.0);

    // Add symbol to subscription
    quote_sub.add_symbols(&[streamer_symbol.clone()]);

    // Wait for a quote
    info!("Waiting for quote data...");

    let mut current_price: Option<Decimal> = None;
    let timeout = tokio::time::Instant::now() + Duration::from_secs(10);

    while current_price.is_none() && tokio::time::Instant::now() < timeout {
        if let Ok(Event { data, .. }) = quote_sub.get_event().await {
            if let EventData::Quote(quote) = data {
                // Use mid price
                let mid_price = Decimal::from_f64((quote.bid_price + quote.ask_price) / 2.0)
                    .unwrap_or_default();
                current_price = Some(mid_price);
                info!(
                    "Current price for {}: ${}",
                    symbol.0,
                    current_price.unwrap()
                );
                break;
            }
        }

        // Brief pause before trying again
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let current_price = match current_price {
        Some(price) => price,
        None => {
            error!("Could not get current price for {}. Aborting.", symbol.0);
            return Ok(());
        }
    };

    // Step 2: Get option chain to find appropriate expiration and strike
    info!("Getting option chain for {}...", symbol.0);
    let chain = tasty.nested_option_chain_for(symbol.clone()).await?;

    info!(
        "Found {} expirations for {}",
        chain.expirations.len(),
        symbol.0
    );

    // Find expiration closest to our target DTE
    let target_expiration = chain
        .expirations
        .iter()
        .min_by_key(|exp| {
            if exp.days_to_expiration < DTE_TARGET {
                u64::MAX - exp.days_to_expiration // Prioritize expirations before target
            } else {
                exp.days_to_expiration - DTE_TARGET
            }
        })
        .ok_or("No valid expirations found")?;

    info!(
        "Selected expiration: {} (DTE: {})",
        target_expiration.expiration_date, target_expiration.days_to_expiration
    );

    // Find the put strike closest to current price
    let target_strike = target_expiration
        .strikes
        .iter()
        .min_by(|a, b| {
            let a_diff = (a.strike_price - current_price).abs();
            let b_diff = (b.strike_price - current_price).abs();
            a_diff.partial_cmp(&b_diff).unwrap()
        })
        .ok_or("No valid strikes found")?;

    info!(
        "Selected strike: ${} (current price: ${})",
        target_strike.strike_price, current_price
    );

    // Get the put option symbol
    let put_symbol = target_strike.put.clone();
    info!("Put option symbol: {}", put_symbol.0);

    // Step 3: Create order leg for short put using OrderLegBuilder
    let order_leg = OrderLegBuilder::default()
        .instrument_type(InstrumentType::EquityOption)
        .symbol(put_symbol.clone())
        .quantity(CONTRACT_QUANTITY)
        .action(Action::SellToOpen)
        .build()?; // Note that build() returns Result, hence the ?

    // Step 4: Create market order for the put using OrderBuilder
    let order = OrderBuilder::default()
        .time_in_force(TimeInForce::Day)
        .order_type(OrderType::Market)
        .price(Decimal::ZERO) // Market order doesn't require a price, but API needs a value
        .price_effect(PriceEffect::Credit) // Selling a put is a credit
        .legs(vec![order_leg])
        .build()?; // Also returns Result

    // Step 5: Do a dry run first to check for errors and see buying power effect
    info!("Performing dry run of order...");
    let dry_run_result = account.dry_run(&order).await?;

    info!("Dry run successful:");
    info!(
        "  Buying power effect: ${} {}",
        dry_run_result.buying_power_effect.change_in_buying_power,
        dry_run_result
            .buying_power_effect
            .change_in_buying_power_effect
    );
    info!(
        "  Estimated fees: ${} {}",
        dry_run_result.fee_calculation.total_fees, dry_run_result.fee_calculation.total_fees_effect
    );

    // Check if there are any warnings
    if !dry_run_result.warnings.is_empty() {
        warn!("Order has {} warnings:", dry_run_result.warnings.len());
        // En una aplicación real, examinarías estas advertencias
    }

    // Step 6: Confirm and place the order
    if config.use_demo {
        info!(
            "\nReady to place market order to sell {} {} put at ${} expiring on {}",
            CONTRACT_QUANTITY,
            symbol.0,
            target_strike.strike_price,
            target_expiration.expiration_date
        );

        info!("Since this is demo mode, proceeding with order placement...");

        // Place the order
        match account.place_order(&order).await {
            Ok(result) => {
                info!("Order placed successfully!");
                info!("Order ID: {}", result.order.id.0);
                info!("Status: {:?}", result.order.status);
                info!(
                    "You have sold {} {} put(s) at strike ${}.",
                    CONTRACT_QUANTITY, symbol.0, target_strike.strike_price
                );
                info!("The premium received should be visible in your account soon.");
            }
            Err(e) => {
                error!("Error placing order: {}", e);
            }
        }
    } else {
        // Este ejemplo requiere confirmación explícita para cuentas reales
        warn!("\n!!! LIVE ACCOUNT ORDER CONFIRMATION REQUIRED !!!");
        warn!("This example does not automatically place orders on live accounts.");
        warn!("If you want to place this order, please:");
        warn!("1. Log into your TastyTrade account");
        warn!(
            "2. Create a market order to sell {} {} put(s) at strike ${}",
            CONTRACT_QUANTITY, symbol.0, target_strike.strike_price
        );
        warn!("Symbol to use: {}", put_symbol.0);
    }

    info!("\nExample completed successfully!");
    Ok(())
}
