//! One account by number, from its own endpoint.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin get_customer_account
//! ```
//!
//! `account()` used to download every account and filter here, which meant a
//! *sibling* account that failed to deserialize took the one you asked for with
//! it — `Items<T>` skips what it cannot parse, so the answer came back
//! `Ok(None)` and looked like "this session cannot see that account". That is
//! the shape of the `is-test-drive` bug. Now `Ok(None)` means the venue said
//! 404 and nothing else does.

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
    let Some(first) = accounts.first() else {
        info!("This session sees no accounts.");
        return Ok(());
    };
    let number = first.number();
    info!("Listing shows account {}", number.redacted());

    // The same account, fetched on its own.
    match tasty.account_by_number(number.clone()).await? {
        Some(account) => {
            let details = account.details();
            info!(
                "Single fetch: {} — {} ({}, opened {})",
                account.number().redacted(),
                details.nickname,
                details.account_type_name,
                details.opened_at
            );
            info!("  margin or cash: {}", details.margin_or_cash);
            // A flag the venue omitted is unknown, never false.
            info!("  futures approved: {:?}", details.is_futures_approved);
        }
        None => info!("The venue does not know that account number"),
    }

    // A number that cannot exist: `Ok(None)`, from a real 404 rather than from
    // a sibling that failed to parse.
    match tasty.account_by_number("0XX00000").await? {
        Some(_) => info!("A made-up account number answered, which is surprising"),
        None => info!("A made-up account number resolves to None"),
    }

    Ok(())
}
