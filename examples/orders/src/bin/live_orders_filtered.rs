//! Working orders, narrowed at the venue.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin live_orders_filtered
//! ```
//!
//! The live endpoint takes a **single** `status` and an underlying symbol —
//! not the history filters. Sending those here would be ignored and the caller
//! would believe a full listing had been narrowed, which is why the two
//! filters are different types.

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

    // The no-argument call still works and still returns every working order.
    let all = account.live_orders().await?;
    info!("{} working order(s) unfiltered", all.len());

    let live = account
        .live_orders_matching(
            &LiveOrderFilter::new()
                .with_status(OrderStatus::Live)
                .with_page(PageRequest::first().with_per_page(25)),
        )
        .await?;
    info!(
        "{} live order(s) on page {} of {}",
        live.len(),
        live.pagination.page_offset,
        live.pagination.total_pages
    );
    for order in &live {
        info!(
            "  #{} {} {}",
            order.id.0, order.underlying_symbol.0, order.status
        );
    }

    Ok(())
}
