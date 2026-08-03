//! How much of each instrument type the account may order and hold.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin position_limit
//! ```

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

    let limit = account.position_limit().await?;

    for (name, order, position) in [
        (
            "equity",
            limit.equity_order_size,
            limit.equity_position_size,
        ),
        (
            "equity option",
            limit.equity_option_order_size,
            limit.equity_option_position_size,
        ),
        (
            "future",
            limit.future_order_size,
            limit.future_position_size,
        ),
        (
            "future option",
            limit.future_option_order_size,
            limit.future_option_position_size,
        ),
    ] {
        // `None` means the venue did not report a limit, which is not the same
        // as a limit of zero.
        info!(
            "{name}: order {} / position {}",
            show(order.as_ref()),
            show(position.as_ref())
        );
    }
    info!(
        "opening orders per underlying: {}",
        show(limit.underlying_opening_order_limit.as_ref())
    );

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "unreported".to_string())
}
