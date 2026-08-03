//! One complex order in full.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p orders --bin get_complex_order
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
        .complex_orders(&PageRequest::first().with_per_page(1))
        .await?;
    let Some(id) = page.items.first().and_then(|c| c.id.clone()) else {
        info!("This account has no complex orders yet.");
        return Ok(());
    };

    let container = account.complex_order(&id).await?;

    info!("Complex order {}", id.0);
    info!(
        "  type: {}",
        container
            .complex_order_type
            .as_ref()
            .map(ComplexOrderType::as_wire)
            .unwrap_or("-")
    );
    // The PAIRS threshold, when there is one. `None` means the venue sent
    // nothing, not a threshold of zero.
    info!(
        "  ratio threshold: {:?} {:?}",
        container.ratio_price_threshold, container.ratio_price_comparator
    );
    info!("  components: {}", container.orders.len());
    info!("  related: {}", container.related_orders.len());

    Ok(())
}
