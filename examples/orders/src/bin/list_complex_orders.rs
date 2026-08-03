//! The account's complex orders, page by page.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin list_complex_orders
//! ```
//!
//! Read-only.

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

    let page = account
        .complex_orders(&PageRequest::first().with_per_page(25))
        .await?;

    info!(
        "{} complex order(s) on page {} of {}",
        page.len(),
        page.pagination.page_offset,
        page.pagination.total_pages
    );
    for container in &page {
        info!(
            "  {} {} — {} component(s), {}",
            container
                .id
                .as_ref()
                .map(|id| id.0.as_str())
                .unwrap_or("(no id)"),
            container
                .complex_order_type
                .as_ref()
                .map(ComplexOrderType::as_wire)
                .unwrap_or("(no type)"),
            container.orders.len(),
            // A component whose status this crate has not seen counts as
            // working: an unknown status says nothing about whether it is done.
            if container.has_working_components() {
                "still working"
            } else {
                "all terminal"
            }
        );
    }

    Ok(())
}
