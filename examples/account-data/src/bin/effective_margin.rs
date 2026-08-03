//! The standing margin requirement for one underlying.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin effective_margin
//! ```

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// Underlyings to ask about. The second carries a separator, which is the case
/// the shared path encoder exists for.
const UNDERLYINGS: [&str; 2] = ["AAPL", "BRK/B"];

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

    for underlying in UNDERLYINGS {
        match account.effective_margin_requirement(underlying).await {
            Ok(requirement) => info!(
                "{underlying}: long initial {} / maintenance {}, short initial {} / maintenance {}, \
                 naked option standard {}",
                show(requirement.long_equity_initial.as_ref()),
                show(requirement.long_equity_maintenance.as_ref()),
                show(requirement.short_equity_initial.as_ref()),
                show(requirement.short_equity_maintenance.as_ref()),
                show(requirement.naked_option_standard.as_ref())
            ),
            Err(error) => info!("{underlying}: {error}"),
        }
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
