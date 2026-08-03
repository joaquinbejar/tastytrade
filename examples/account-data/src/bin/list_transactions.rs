//! The account ledger, unfiltered and filtered.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin list_transactions
//! ```
//!
//! Shows both calls the endpoint really has: the whole ledger a page at a time,
//! and a server-side filter narrowing it by date range and transaction kind.

use chrono::{Duration, Utc};
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// How many rows to print per page.
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

    // Unfiltered: the whole ledger, newest first, one venue-sized page.
    let page = account
        .transactions(&TransactionFilter::new().with_page(PageRequest::first().with_per_page(25)))
        .await?;
    info!(
        "{} transaction(s) on page {} of {}, {} in the ledger",
        page.len(),
        page.pagination.page_offset,
        page.pagination.total_pages,
        page.pagination.total_items
    );
    for transaction in page.iter().take(MAX_ROWS) {
        print_row(transaction);
    }

    // Filtered at the venue: a date range plus several kinds. `types` and
    // `type` are mutually exclusive there, so they are one enum here.
    let filtered = account
        .transactions(
            &TransactionFilter::new()
                .with_types(TransactionTypes::Several(vec![
                    TransactionType::Trade,
                    TransactionType::ReceiveDeliver,
                ]))
                .with_dates(
                    Some((Utc::now() - Duration::days(30)).date_naive()),
                    Some(Utc::now().date_naive()),
                )
                .with_sort(TransactionSort::Ascending),
        )
        .await?;
    info!(
        "{} trade or receive-deliver transaction(s) in the last 30 days",
        filtered.pagination.total_items
    );
    for transaction in filtered.iter().take(MAX_ROWS) {
        print_row(transaction);
    }

    Ok(())
}

/// One ledger row, without the account number.
///
/// `description` is venue prose written for a person — it can name an amount or
/// an instrument — so it belongs on somebody's screen and not in a log
/// aggregator. Printed here because that is what this example is for.
fn print_row(transaction: &Transaction) {
    info!(
        "  #{} {} / {} — {} {} net {} {}",
        transaction.id,
        render(transaction.transaction_type.as_ref()),
        render(transaction.transaction_sub_type.as_ref()),
        transaction.symbol.as_deref().unwrap_or("(no symbol)"),
        transaction
            .quantity
            .map(|quantity| quantity.to_string())
            .unwrap_or_else(|| "-".to_string()),
        transaction
            .net_value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        transaction
            .net_value_effect
            .map(|effect| effect.to_string())
            .unwrap_or_default(),
    );
}

/// A wire enum's own text, or a dash. A value this crate has not seen keeps the
/// venue's spelling rather than disappearing.
fn render<T: std::fmt::Display>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_string())
}
