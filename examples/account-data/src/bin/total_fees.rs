//! What an account paid in fees, today and on a named day.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin total_fees
//! ```
//!
//! The omitted-date call is the interesting one: leaving the parameter out is
//! not the same as sending today's date, because "today" is the venue's
//! decision and this process may not be in its timezone.

use chrono::{Duration, Utc};
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

    let tasty = TastyTrade::connect(&config).await?;
    let accounts = tasty.accounts().await?;
    let Some(account) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    info!("Account {}", account.number().redacted());

    // No date: the venue answers for its own today.
    let today = account.total_fees(None).await?;
    // Stdout: a fee total is account financial data, and INFO reaches the
    // consuming application's aggregator by default.
    println!("Today: {}", render(&today));

    // An explicit day.
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();
    let named = account.total_fees(Some(yesterday)).await?;
    println!("{yesterday}: {}", render(&named));

    Ok(())
}

/// The total with its direction.
///
/// The amount is a magnitude: a debit of 100 and a credit of 100 are opposite
/// facts about the same number, so printing one without the other is printing
/// half an answer.
fn render(fees: &TotalFees) -> String {
    match (fees.total_fees, fees.total_fees_effect) {
        (Some(amount), Some(effect)) => format!("{amount} ({effect})"),
        (Some(amount), None) => format!("{amount} (direction unstated)"),
        (None, _) => "the venue reported no total".to_string(),
    }
}
