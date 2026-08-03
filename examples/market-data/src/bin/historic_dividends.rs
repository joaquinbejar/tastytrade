//! An underlying's dividend history.
//!
//! **Live only**, like the rest of Market Metrics:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false TASTYTRADE_ALLOW_PRODUCTION_READ=1 \
//!   cargo run -p market-data --bin historic_dividends
//! ```

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

    // The second symbol carries a class separator, which is the case the shared
    // path encoder exists for.
    for symbol in ["AAPL", "BRK/B"] {
        match tasty.historic_dividends(symbol).await {
            Ok(dividends) => {
                info!("{symbol}: {} dividend(s)", dividends.len());
                for dividend in dividends.iter().take(10) {
                    info!(
                        "  {}: {}",
                        dividend
                            .occurred_date
                            .map(|date| date.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        dividend
                            .amount
                            .map(|amount| amount.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            Err(error) => info!("{symbol}: {error}"),
        }
    }

    Ok(())
}
