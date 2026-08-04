//! What this OAuth grant is actually allowed to read.
//!
//! [#125](https://github.com/joaquinbejar/tastytrade/issues/125) wants a real
//! captured payload per endpoint, to replace fixtures that were written from
//! the same document the types were derived from. Whether that is possible is
//! not a property of the crate — it is a property of the **grant**, and the
//! answer differs per route: an application scoped for one listing is refused
//! on the next.
//!
//! So this asks, once, read-only, and prints one row per route. It is the map
//! that says which fixtures can be captured today and which are waiting on
//! wider scopes.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin probe_entitlements
//! ```
//!
//! **Shape, never content.** Status and the count of items, nothing from
//! inside a record. Some of these routes return an account's own data.

use serde_json::Value;
use tastytrade::api::base::Items;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// Read-only routes, one per area the crate covers.
///
/// Account-scoped paths are filled in from the listing when it answers, so the
/// account number never appears in this file.
const ROUTES: [&str; 18] = [
    "/customers/me",
    "/customers/me/accounts",
    "/instruments/equities?per-page=1",
    "/instruments/cryptocurrencies",
    "/instruments/warrants",
    "/instruments/futures?per-page=1",
    "/instruments/future-products?per-page=1",
    "/instruments/quantity-decimal-precisions",
    "/instruments/equity-options?per-page=1",
    "/option-chains/SPY/nested",
    "/symbols/search/SPY",
    "/instruments/search?query=SPY",
    "/market-metrics?symbols=SPY",
    "/market-data/by-type?equity=SPY",
    "/market-time/equities/sessions/current",
    "/quote-alerts",
    "/watchlists",
    "/pairs-watchlists",
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    println!("Entitlements on {} (read-only)\n", config.environment());

    let mut readable = 0;
    for path in ROUTES {
        let outcome = probe(&tasty, path).await;
        if outcome.starts_with("200") {
            readable += 1;
        }
        println!("{outcome:<28} {path}");
    }

    println!("\n{readable}/{} readable under this grant", ROUTES.len());
    println!("Routes answering 200 are the ones #125 can capture today.");

    Ok(())
}

/// One route, described by its status and how much came back.
async fn probe(tasty: &TastyTrade, path: &str) -> String {
    // Tries the listing envelope first so a count is available, then the bare
    // object for the single-resource routes.
    if let Ok(items) = tasty.get::<Items<Value>, _>(path).await {
        return format!("200 ({} item(s))", items.items.len());
    }

    match tasty.get::<Value, _>(path).await {
        Ok(_) => "200 (single object)".to_string(),
        Err(TastyTradeError::Request { context, api }) => {
            let code = api
                .as_ref()
                .and_then(|error| error.code.as_deref())
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default();
            match context.status {
                Some(status) => format!("{status}{code}"),
                None => "no response".to_string(),
            }
        }
        Err(TastyTradeError::Precondition(_)) => "refused locally".to_string(),
        Err(_) => "failed before a status".to_string(),
    }
}
