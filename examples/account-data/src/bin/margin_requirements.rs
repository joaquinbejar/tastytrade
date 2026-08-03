//! The account's standing margin requirements, by underlying and by strategy.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin margin_requirements
//! ```

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

    let report = account.margin_requirements().await?;

    info!(
        "Total margin {} {}, maintenance {} {}",
        show(report.margin_requirement.as_ref()),
        show(report.margin_requirement_effect.as_ref()),
        show(report.maintenance_requirement.as_ref()),
        show(report.maintenance_requirement_effect.as_ref())
    );
    info!(
        "Option buying power {}, maintenance excess {}",
        show(report.option_buying_power.as_ref()),
        show(report.maintenance_excess.as_ref())
    );

    // The nesting is the point: the per-strategy figures are what explain the
    // total, and flattening them would leave a number with no reason.
    for group in &report.groups {
        info!(
            "  {} ({}): margin {}, maintenance {}",
            show(group.underlying_symbol.as_ref()),
            show(group.underlying_type.as_ref()),
            show(group.margin_requirement.as_ref()),
            show(group.maintenance_requirement.as_ref())
        );
        for strategy in &group.groups {
            info!(
                "    {}: margin {} over {} position(s)",
                show(strategy.description.as_ref()),
                show(strategy.margin_requirement.as_ref()),
                strategy.position_entries.len()
            );
            for entry in &strategy.position_entries {
                info!(
                    "      {} {} @ close {} (fixing {})",
                    show(entry.instrument_symbol.as_ref()),
                    show(entry.quantity.as_ref()),
                    show(entry.close_price.as_ref()),
                    // `None` here is the venue's `NaN`: this instrument does
                    // not fix, which is not a fixing price of zero.
                    show(entry.fixing_price.as_ref())
                );
            }
        }
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
