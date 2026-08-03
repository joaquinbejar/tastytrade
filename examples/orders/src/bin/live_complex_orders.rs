//! Complex orders with components placed today.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin live_complex_orders
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

    for container in account.live_complex_orders().await? {
        info!(
            "{} {}",
            container
                .id
                .as_ref()
                .map(|id| id.0.as_str())
                .unwrap_or("(no id)"),
            container
                .complex_order_type
                .as_ref()
                .map(ComplexOrderType::as_wire)
                .unwrap_or("(no type)")
        );
        for component in &container.orders {
            info!(
                "  {} [{}] {} {}",
                component.id.as_deref().unwrap_or("-"),
                component.complex_order_tag.as_deref().unwrap_or("-"),
                component
                    .underlying_symbol
                    .as_ref()
                    .map(|symbol| symbol.0.as_str())
                    .unwrap_or("-"),
                component
                    .status
                    .as_ref()
                    .map(OrderStatus::as_wire)
                    .unwrap_or("-")
            );
        }
        for related in &container.related_orders {
            info!(
                "  (related) {} {}",
                related.id.as_deref().unwrap_or("-"),
                related
                    .status
                    .as_ref()
                    .map(OrderStatus::as_wire)
                    .unwrap_or("-")
            );
        }
    }

    Ok(())
}
