//! The account's order history, filtered at the venue.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin search_orders
//! ```
//!
//! Read-only: nothing here places, replaces or cancels anything.

use chrono::{Duration, Utc};
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
    let Some(account) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    info!("Account {}", account.number().redacted());
    // Order contents go to stdout, not through `tracing`. Underlying, price,
    // leg symbols, quantities and actions are what an account is doing with
    // its money, and INFO is the default level that reaches whatever
    // aggregator the consuming application configured.

    // Terminal statuses over the last month, oldest first. `status[]` is a
    // repeated key here, unlike the live endpoint's singular `status`.
    let page = account
        .search_orders(
            &OrderFilter::new()
                .with_statuses(&[OrderStatus::Filled, OrderStatus::Cancelled])
                .with_dates(
                    Some((Utc::now() - Duration::days(30)).date_naive()),
                    Some(Utc::now().date_naive()),
                )
                .with_sort(OrderSort::Ascending)
                .with_page(PageRequest::first().with_per_page(25)),
        )
        .await?;

    println!(
        "{} order(s) on page {} of {}, {} in the history",
        page.len(),
        page.pagination.page_offset,
        page.pagination.total_pages,
        page.pagination.total_items
    );
    for order in page.iter().take(MAX_ROWS) {
        println!(
            "  #{} {} {} — {} ({} legs)",
            order.id.0,
            order.underlying_symbol.0,
            order.status,
            if order.status.is_terminal() {
                "terminal"
            } else {
                "still working"
            },
            order.legs.len()
        );
    }

    Ok(())
}
