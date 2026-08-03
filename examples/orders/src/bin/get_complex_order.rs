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
    // Component symbols, buying-power effects, order identifiers and the
    // venue's warning prose go to stdout. A warning can name the account or
    // the buying power it is about, and INFO is the default level that reaches
    // whatever aggregator the consuming application configured.

    let page = account
        .complex_orders(&PageRequest::first().with_per_page(1))
        .await?;
    let Some(id) = page.items.first().and_then(|c| c.id.clone()) else {
        println!("This account has no complex orders yet.");
        return Ok(());
    };

    let container = account.complex_order(&id).await?;

    println!("Complex order {}", id.0);
    println!(
        "  type: {}",
        container
            .complex_order_type
            .as_ref()
            .map(ComplexOrderType::as_wire)
            .unwrap_or("-")
    );
    // The PAIRS threshold, when there is one. `None` means the venue sent
    // nothing, not a threshold of zero.
    println!(
        "  ratio threshold: {:?} {:?}",
        container.ratio_price_threshold, container.ratio_price_comparator
    );
    println!("  components: {}", container.orders.len());
    println!("  related: {}", container.related_orders.len());

    Ok(())
}
