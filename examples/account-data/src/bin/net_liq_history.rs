//! The account's equity curve.
//!
//! **Live only.** The venue's sandbox page lists Net Liq History as
//! unavailable in certification, so this example requires an explicit
//! read-only production opt-in and refuses to guess:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false TASTYTRADE_ALLOW_PRODUCTION_READ=1 \
//!   cargo run -p account-data --bin net_liq_history
//! ```
//!
//! It never places, replaces or cancels anything. The opt-in exists because
//! "this endpoint only works in production" must not become "this example
//! quietly points at production".

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// The variable that has to be set on purpose.
const OPT_IN: &str = "TASTYTRADE_ALLOW_PRODUCTION_READ";

/// How many bars to print.
const MAX_ROWS: usize = 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    // Two separate gates on purpose. The first says which deployment the
    // configuration selected; the second says somebody chose to read from it.
    // Neither is inferred from the other.
    info!("Environment: {}", config.environment());
    if config.environment() != Environment::Production {
        info!(
            "Net Liq History is not served in certification. Re-run with \
             TASTYTRADE_USE_DEMO=false and {OPT_IN}=1 to read it from production."
        );
        return Ok(());
    }
    if std::env::var(OPT_IN).is_err() {
        info!(
            "This would read from PRODUCTION. Set {OPT_IN}=1 to allow it. \
             Nothing is placed or modified either way."
        );
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let accounts = tasty.accounts().await?;
    let Some(account) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    info!("Account {} — READ ONLY", account.number().redacted());

    // Relative to now. The window form is the other half of the enum, and the
    // two can never be sent together.
    let bars = account
        .net_liq_history(&NetLiqHistoryFilter::back(TimeBack::OneMonth))
        .await?;

    info!("{} bar(s) over the last month", bars.len());
    for bar in bars.iter().take(MAX_ROWS) {
        info!(
            "  {}: o {} h {} l {} c {} (total close {})",
            // `time` is exactly what the venue sent: its schema gives it no
            // format, and the same service documents JVM `ZonedDateTime` for
            // its inputs, which is not RFC 3339.
            show(bar.time.as_ref()),
            show(bar.open.as_ref()),
            show(bar.high.as_ref()),
            show(bar.low.as_ref()),
            show(bar.close.as_ref()),
            show(bar.total_close.as_ref())
        );
    }

    // A drawdown needs the peak, which is what the curve is for.
    let peak = bars.iter().filter_map(|bar| bar.high).max();
    let last = bars.iter().rev().find_map(|bar| bar.close);
    if let (Some(peak), Some(last)) = (peak, last) {
        info!("Peak {peak}, latest {last}, drawdown {}", peak - last);
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
