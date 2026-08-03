//! An underlying's earnings history over a range.
//!
//! **Live only**, like the rest of Market Metrics:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false TASTYTRADE_ALLOW_PRODUCTION_READ=1 \
//!   cargo run -p market-data --bin historic_earnings
//! ```
//!
//! `start-date` is required by the venue, so `EarningsRange` carries it and it
//! cannot be omitted — a required query parameter should be impossible to leave
//! out rather than a runtime 400.

use chrono::{Duration, Utc};
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

const OPT_IN: &str = "TASTYTRADE_ALLOW_PRODUCTION_READ";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }
    info!("Environment: {}", config.environment());
    if config.environment() != Environment::Production || std::env::var(OPT_IN).is_err() {
        info!(
            "Market Metrics is live only. Re-run with TASTYTRADE_USE_DEMO=false and \
             {OPT_IN}=1 to read it from production."
        );
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    let two_years = EarningsRange::between(
        (Utc::now() - Duration::days(730)).date_naive(),
        Utc::now().date_naive(),
    );
    let reports = tasty.historic_earnings("AAPL", &two_years).await?;

    info!(
        "{} earnings report(s) since {}",
        reports.len(),
        two_years.start_date()
    );
    for report in &reports {
        info!(
            "  {}: eps {}",
            report
                .occurred_date
                .map(|date| date.to_string())
                .unwrap_or_else(|| "-".to_string()),
            // A loss is a real figure, not a missing one.
            report
                .eps
                .map(|eps| eps.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
    }

    // Open-ended: from a date onwards, with no end.
    let open = EarningsRange::from((Utc::now() - Duration::days(365)).date_naive());
    info!(
        "{} report(s) since {} with no end date",
        tasty.historic_earnings("AAPL", &open).await?.len(),
        open.start_date()
    );

    Ok(())
}
