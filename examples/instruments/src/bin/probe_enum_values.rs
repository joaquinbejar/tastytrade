//! Which values the venue actually puts in the fields this crate left as text.
//!
//! Four fields are typed `String` on purpose — `option-chain-type`,
//! `option-type`, `product-type` and `margin-or-cash` — because no captured
//! payload showed their value sets and guessing a closed set from a field name
//! produces variants that never match. That reasoning is only as good as its
//! premise, and [#125](https://github.com/joaquinbejar/tastytrade/issues/125)
//! is about replacing the premise with observation.
//!
//! This counts the distinct values across as many real records as the grant can
//! read, and prints how many records each count is drawn from. A value set from
//! one record is not a value set.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin probe_enum_values
//! ```
//!
//! **Shape, never content.** Field values of classification fields only —
//! never a symbol, a balance or an account number.

use std::collections::BTreeMap;

use serde_json::Value;
use tastytrade::api::base::Items;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// The field to census, and the listing to read it from.
const CENSUS: [(&str, &str); 7] = [
    ("product-type", "/instruments/future-products?per-page=1000"),
    ("margin-or-cash", "/customers/me/accounts"),
    // The classification lives on the chain, not on the underlying listing.
    ("option-chain-type", "/option-chains/SPY/nested"),
    ("option-type", "/option-chains/SPY/nested"),
    ("expiration-type", "/option-chains/SPY/nested"),
    ("settlement-type", "/option-chains/SPY/nested"),
    ("lendability", "/instruments/equities?per-page=1000"),
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
    println!("Value census on {}\n", config.environment());

    for (field, path) in CENSUS {
        match tasty.get::<Items<Value>, _>(path).await {
            Ok(items) => {
                let counts = census(&items.items, field);
                let seen: usize = counts.values().sum();
                println!(
                    "{field}  — {} record(s) read, {seen} carried it",
                    items.items.len()
                );
                if counts.is_empty() {
                    println!("    (no record carried this field)");
                }
                for (value, n) in counts {
                    println!("    {n:>6}  {value}");
                }
            }
            Err(error) => println!("{field}  — unreadable: {error}"),
        }
        println!();
    }

    println!("A set drawn from one record is not a set. Narrow only what the");
    println!("counts actually support, and note the rest as still unobserved.");

    Ok(())
}

/// How many records carry each distinct value of `field`.
///
/// Looks one level down as well, since the option-chain classification sits on
/// the nested option-chain object rather than on the instrument itself.
fn census(items: &[Value], field: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        for value in find(item, field) {
            *counts.entry(value).or_insert(0) += 1;
        }
    }
    counts
}

/// Every string value stored under `field`, at any depth.
fn find(node: &Value, field: &str) -> Vec<String> {
    let mut out = Vec::new();
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if key == field {
                    if let Some(text) = value.as_str() {
                        out.push(text.to_string());
                    }
                }
                out.extend(find(value, field));
            }
        }
        Value::Array(items) => {
            for item in items {
                out.extend(find(item, field));
            }
        }
        _ => {}
    }
    out
}
