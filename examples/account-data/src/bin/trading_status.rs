//! Whether the account may trade, before finding out from a rejection.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin trading_status
//! ```
//!
//! One request, and it answers the question a dry-run answers expensively: a
//! closed or frozen account cannot trade at all, a closing-only account can
//! only reduce, and the feature flags decide whether futures, cryptocurrency
//! or uncovered short calls are even available.

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

    let status = account.trading_status().await?;

    // The three that decide whether an order is worth sending.
    info!("Blocked from trading: {}", status.is_blocked());
    info!("Reduce only: {}", status.is_reduce_only());
    if !status.is_known_blocked() {
        // The distinction the `Option<bool>` fields exist to preserve: the
        // venue did not send the flags, so "not blocked" is not something this
        // process actually knows.
        info!("  …but the venue did not send both flags, so that is unverified");
    }

    info!("Options level: {}", show(status.options_level.as_ref()));
    info!(
        "Day trades used today: {}",
        show(status.day_trade_count.as_ref())
    );
    info!(
        "Margin calculation: {}",
        show(status.equities_margin_calculation_type.as_ref())
    );

    // Feature flags. `None` means unstated, which is not "no".
    for (name, flag) in [
        ("futures", status.is_futures_enabled),
        ("cryptocurrency", status.is_cryptocurrency_enabled),
        ("equity offerings", status.is_equity_offering_enabled),
        ("uncovered short calls", status.short_calls_enabled),
        ("portfolio margin", status.is_portfolio_margin_enabled),
    ] {
        info!("  {name}: {}", flag_text(flag));
    }

    // Restrictions worth knowing before an order is refused.
    for (name, flag) in [
        ("in a margin call", status.is_in_margin_call),
        ("pattern day trader", status.is_pattern_day_trader),
        ("frozen", status.is_frozen),
        ("closed", status.is_closed),
        ("closing only", status.is_closing_only),
    ] {
        if flag == Some(true) {
            info!("  restriction: {name}");
        }
    }

    // Cryptocurrency is enabled at the account level and separately disabled
    // for the API venue-wide as of 2026-06-29, so this flag being true does not
    // mean an API crypto order will route.
    if status.is_cryptocurrency_enabled == Some(true) {
        info!(
            "Note: cryptocurrency is enabled on the account, but tastytrade \
             disabled crypto trading through the API on 2026-06-29"
        );
    }

    Ok(())
}

/// A value, or a note that the venue did not send one.
fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "unstated".to_string())
}

/// A flag, keeping "unstated" distinct from "no".
fn flag_text(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unstated",
    }
}
