//! One transaction, in full.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin get_transaction
//! ```
//!
//! Takes the newest row from the ledger and fetches it on its own, which is
//! how a caller reconciles a fill: the listing says what happened, this says
//! exactly what it cost.

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
        .transactions(&TransactionFilter::new().with_page(PageRequest::first().with_per_page(1)))
        .await?;
    let Some(newest) = page.items.first() else {
        info!("This account has no transactions yet.");
        return Ok(());
    };

    let transaction = account.transaction(newest.id).await?;

    // Stdout, not `tracing`. The rest of this is one account's trade: symbol,
    // quantity, price, commission, fees and net value. INFO is the default
    // level and reaches whatever aggregator the consuming application set up,
    // so the values go to the destination somebody chose by running this.
    println!("Transaction #{}", transaction.id);
    println!(
        "  {} / {}",
        show(transaction.transaction_type.as_ref()),
        show(transaction.transaction_sub_type.as_ref())
    );
    println!("  symbol: {}", transaction.symbol.as_deref().unwrap_or("-"));
    println!("  executed at: {}", show(transaction.executed_at.as_ref()));
    // Every one of these is `None` when the venue did not send it, which is not
    // the same as zero — a commission that defaults to zero is a P&L that is
    // quietly wrong.
    println!("  gross value: {}", show(transaction.value.as_ref()));
    println!("  commission: {}", show(transaction.commission.as_ref()));
    println!(
        "  regulatory fees: {}",
        show(transaction.regulatory_fees.as_ref())
    );
    println!(
        "  clearing fees: {}",
        show(transaction.clearing_fees.as_ref())
    );
    println!("  net value: {}", show(transaction.net_value.as_ref()));
    println!(
        "  fees are an estimate: {}",
        match transaction.is_estimated_fee {
            Some(true) => "yes",
            Some(false) => "no",
            None => "unstated",
        }
    );

    Ok(())
}

/// A value, or a dash when the venue did not send one.
fn show<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
