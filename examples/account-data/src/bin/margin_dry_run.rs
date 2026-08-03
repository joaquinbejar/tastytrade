//! What one order would do to buying power — without routing it.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin margin_dry_run
//! ```
//!
//! `estimate_margin` is **not** the order preflight. `Account::dry_run` asks
//! "would the venue accept this order"; this asks "how much buying power would
//! it take". Neither routes anything, and there is no path from this one to a
//! placement — it takes its own request type, which is why an `Order` cannot
//! be handed to it by accident.

use rust_decimal::Decimal;
use std::str::FromStr;
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
    // Values go to stdout, not through `tracing`. Margin, buying power,
    // symbols, quantities and close prices are account financial data, and
    // INFO is the default level that reaches whatever aggregator the consuming
    // application configured. Stdout is the destination somebody chose by
    // running this example.

    let request = MarginOrderRequest::new(
        account.number().0,
        "AAPL",
        InstrumentType::Equity,
        OrderType::Limit,
        TimeInForce::Day,
        vec![MarginOrderLeg {
            symbol: "AAPL".to_string(),
            instrument_type: InstrumentType::Equity,
            quantity: Decimal::from(1),
            action: Action::BuyToOpen,
            remaining_quantity: None,
        }],
    )
    .with_price(Decimal::from_str("186.99")?, PriceEffect::Debit);

    let estimate = account.estimate_margin(&request).await?;

    println!(
        "Buying power now {}, after {}",
        show(estimate.current_buying_power.as_ref()),
        show(estimate.new_buying_power.as_ref())
    );
    println!(
        "Change in margin {} {}, change in buying power {} {}",
        show(estimate.change_in_margin_requirement.as_ref()),
        show(estimate.change_in_margin_requirement_effect.as_ref()),
        show(estimate.change_in_buying_power.as_ref()),
        show(estimate.change_in_buying_power_effect.as_ref())
    );
    if let Some(new) = &estimate.new_order_results {
        println!(
            "With the order: margin {}, maintenance {}, impact {}",
            show(new.margin_requirement.as_ref()),
            show(new.maintenance_requirement.as_ref()),
            show(new.buying_power_impact.as_ref())
        );
    }

    // The local refusals, none of which reach the network.
    let five_legs = MarginOrderRequest::new(
        account.number().0,
        "AAPL",
        InstrumentType::Equity,
        OrderType::Limit,
        TimeInForce::Day,
        (0..5)
            .map(|i| MarginOrderLeg {
                symbol: format!("SYM{i}"),
                instrument_type: InstrumentType::Equity,
                quantity: Decimal::from(1),
                action: Action::BuyToOpen,
                remaining_quantity: None,
            })
            .collect(),
    );
    match account.estimate_margin(&five_legs).await {
        Ok(_) => println!("five legs were accepted, which is a bug"),
        Err(error) => println!(
            "five legs refused locally, retryable: {} — {error}",
            error.is_retryable()
        ),
    }

    Ok(())
}

fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
