//! One order by its identifier.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin get_order
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
        .search_orders(&OrderFilter::new().with_page(PageRequest::first().with_per_page(1)))
        .await?;
    let Some(newest) = page.items.first() else {
        info!("This account has no orders yet.");
        return Ok(());
    };

    let order = account.order(newest.id).await?;

    info!("Order #{} — {}", order.id.0, order.status);
    info!("  underlying: {}", order.underlying_symbol.0);
    info!(
        "  type: {:?}, time in force: {:?}",
        order.order_type, order.time_in_force
    );
    // `price` is `Option` because a market order has none — not because it
    // might be zero.
    info!("  price: {:?} {:?}", order.price, order.price_effect);
    info!(
        "  cancellable: {}, editable: {}",
        order.cancellable, order.editable
    );
    for leg in &order.legs {
        info!("    {} {} {:?}", leg.symbol.0, leg.quantity, leg.action);
    }

    Ok(())
}
