//! Every quote alert this user has set.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p market-data --bin list_quote_alerts
//! ```
//!
//! Read-only. Alerts are per **user**, not per account, which is why this hangs
//! off the client rather than off an account.

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

    let alerts = tasty.quote_alerts().await?;
    info!("{} alert(s)", alerts.len());

    for alert in &alerts {
        info!(
            "  {} {} {} {} — created {}, triggered {}",
            alert.alert_external_id.as_deref().unwrap_or("-"),
            alert
                .symbol
                .as_ref()
                .map(|symbol| symbol.0.as_str())
                .unwrap_or("-"),
            alert
                .operator
                .as_ref()
                .map(QuoteAlertOperator::as_wire)
                .unwrap_or("-"),
            alert.threshold.as_deref().unwrap_or("-"),
            show(alert.created_at.as_ref()),
            // Which timestamps are present is what tells a waiting alert from
            // a fired one.
            show(alert.triggered_at.as_ref())
        );
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
