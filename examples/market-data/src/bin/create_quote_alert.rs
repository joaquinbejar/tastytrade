//! Set a quote alert, then take it back off.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin create_quote_alert
//! ```
//!
//! **Creates user state**, so it refuses to run anywhere but certification —
//! and it cleans up after itself, because an example that leaves an alert
//! behind is an example that fires at somebody later.

use rust_decimal::Decimal;
use std::str::FromStr;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    if config.environment() != Environment::Certification {
        info!("This example creates an alert and runs against certification only.");
        info!("Set TASTYTRADE_USE_DEMO=true.");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    // A threshold far from any plausible quote, so it does not fire before the
    // cleanup below runs.
    let alert = NewQuoteAlert::new(
        "AAPL",
        QuoteAlertField::Last,
        QuoteAlertOperator::Above,
        Decimal::from_str("99999.00")?,
    )
    .with_instrument_type("Equity");

    let created = tasty.create_quote_alert(&alert).await?;
    let Some(id) = created.alert_external_id.clone() else {
        info!("The venue returned an alert with no id, so it cannot be cleaned up.");
        return Ok(());
    };
    info!("Created alert {id}");

    // A zero threshold never reaches the network: it would fire on the first
    // quote, which is almost always a caller who forgot to set it.
    let zero = NewQuoteAlert::new(
        "AAPL",
        QuoteAlertField::Bid,
        QuoteAlertOperator::Below,
        Decimal::ZERO,
    );
    match tasty.create_quote_alert(&zero).await {
        Ok(_) => info!("a zero threshold was accepted, which is a bug"),
        Err(error) => info!(
            "zero threshold refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    // Clean up.
    tasty.cancel_quote_alert(&id).await?;
    info!("Cancelled alert {id}");

    Ok(())
}
