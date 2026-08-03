//! Working orders across several accounts at once.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin customer_live_orders
//! ```
//!
//! Read-only. The live customer endpoint takes `account-numbers[]` and
//! pagination, and nothing else — which is why its filter is a different type
//! from the history one.

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
    let Some(first) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };

    let rest: Vec<_> = accounts.iter().skip(1).map(|a| a.number()).collect();

    let page = tasty
        .customer_live_orders(
            &CustomerLiveOrderFilter::for_accounts(first.number(), &rest)
                .with_page(PageRequest::first().with_per_page(25)),
        )
        .await?;

    info!(
        "{} working order(s) across {} account(s)",
        page.len(),
        1 + rest.len()
    );
    for order in &page {
        info!(
            "  {} #{} {} {}",
            order.account_number.redacted(),
            order.id.0,
            order.underlying_symbol.0,
            order.status
        );
    }

    Ok(())
}
