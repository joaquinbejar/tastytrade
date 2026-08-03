//! Order history across several accounts at once.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin customer_orders
//! ```
//!
//! `account-numbers[]` is required by the venue, so the filter cannot be built
//! without at least one account: the constructor takes a first and any others
//! separately. A `Vec` that happened to be empty would compile and then 400.
//!
//! Read-only.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

const MAX_ROWS: usize = 10;

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
    info!("Searching across {} account(s)", 1 + rest.len());

    let page = tasty
        .customer_orders(
            &CustomerOrderFilter::for_accounts(first.number(), &rest)
                .with_statuses(&[OrderStatus::Filled])
                .with_page(PageRequest::first().with_per_page(25)),
        )
        .await?;

    info!(
        "{} filled order(s) across the accounts",
        page.pagination.total_items
    );
    for order in page.iter().take(MAX_ROWS) {
        info!(
            "  {} #{} {}",
            order.account_number.redacted(),
            order.id.0,
            order.underlying_symbol.0
        );
    }

    Ok(())
}
