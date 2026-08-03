//! The whole balances-and-positions contract, against one account.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p accounts-status --bin balances_and_positions
//! ```
//!
//! Read-only throughout. Everything printed goes through
//! [`AccountNumber::redacted`], because an account number in a terminal ends up
//! in a screenshot.

use chrono::{Duration, Utc};
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// How many positions to print. A funded account can hold hundreds.
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

    // 1. Every balance row. The endpoint answers with a list — one row per
    //    currency — which is why `balance()` alone is not the whole answer.
    let balances = account.balances().await?;
    info!("{} balance row(s)", balances.len());
    for balance in &balances {
        info!(
            "  {}: cash {} net liq {}",
            balance.currency.as_deref().unwrap_or("unnamed"),
            balance.cash_balance,
            balance.net_liquidating_value
        );
    }

    // 2. The single-balance shortcut, which is honest about not applying when
    //    the account holds more than one currency.
    match account.balance().await {
        Ok(balance) => info!("Single balance row: cash {}", balance.cash_balance),
        Err(error) => info!("No single balance: {error}"),
    }

    // 3. One currency by its own route, rather than filtering the list here.
    if let Some(currency) = balances
        .first()
        .and_then(|balance| balance.currency.as_deref())
    {
        let one = account.balance_in(currency).await?;
        info!("{currency} cash from its own route: {}", one.cash_balance);
    }

    // 4. A single day. `snapshot-date` and the range are alternatives, and the
    //    type makes sending both impossible.
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();
    let one_day = account
        .balance_snapshots(
            &BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
                .with_range(SnapshotRange::on(yesterday)),
        )
        .await?;
    info!(
        "{} snapshot(s) for {yesterday}, {} in the listing",
        one_day.len(),
        one_day.pagination.total_items
    );

    // 5. A range, paged.
    let month = account
        .balance_snapshots(
            &BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
                .with_range(SnapshotRange::between(
                    (Utc::now() - Duration::days(30)).date_naive(),
                    Utc::now().date_naive(),
                ))
                .with_page(PageRequest::first().with_per_page(5)),
        )
        .await?;
    info!(
        "{} snapshot(s) on page {} of {}",
        month.len(),
        month.pagination.page_offset,
        month.pagination.total_pages
    );
    for snapshot in month.iter().take(MAX_ROWS) {
        info!(
            "  {} {:?}: net liq {}",
            snapshot.snapshot_date, snapshot.time_of_day, snapshot.net_liquidating_value
        );
    }

    // 6. Positions, filtered at the venue rather than downloaded and filtered
    //    here. `with_marks` is what fills in `mark` and `mark_price`.
    let open = account
        .positions_matching(&PositionFilter::new().with_marks(true))
        .await?;
    info!("{} open position(s)", open.len());
    for position in open.iter().take(MAX_ROWS) {
        info!(
            "  {} {:?} {} — mark {:?}",
            position.symbol.0, position.quantity_direction, position.quantity, position.mark
        );
    }

    // Narrowing server-side: one underlying, closed positions included.
    if let Some(underlying) = open
        .first()
        .map(|position| position.underlying_symbol.clone())
    {
        let narrowed = account
            .positions_matching(
                &PositionFilter::new()
                    .with_underlying_symbols(&[&underlying.0])
                    .with_closed_positions(true),
            )
            .await?;
        info!(
            "{} position(s) on {} including closed",
            narrowed.len(),
            underlying.0
        );
    }

    Ok(())
}
