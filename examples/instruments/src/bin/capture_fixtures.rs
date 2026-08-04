//! Captures real certification responses and writes them to `Doc/captures/`.
//!
//! Every serde fixture in this crate was written by hand from the same OpenAPI
//! document the types were derived from, so decoding one proves the two agree
//! with each other and nothing about what the venue sends
//! ([#130](https://github.com/joaquinbejar/tastytrade/issues/130)).
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin capture_fixtures
//! ```
//!
//! Read-only `GET`s. Nothing here mutates, and the crate resolves anything
//! other than the literal `TASTYTRADE_USE_DEMO=false` to certification.
//!
//! # What gets written, and what does not
//!
//! A capture is a file in the repository, so redaction happens **before** the
//! bytes reach the disk rather than as a later pass over them. Which redaction
//! depends on what the route returns, and the three cases are genuinely
//! different rather than three strengths of the same thing:
//!
//! - [`Redaction::None`] — instruments, option chains, market sessions. Public
//!   reference data about tradeable products. Nothing in these records belongs
//!   to anybody, so they are stored exactly as they arrived.
//! - [`Redaction::Account`] — the account listing. The number and the nickname
//!   identify a person; the rest is structural flags that decide how the type
//!   must be written, so only those two are replaced.
//! - [`Redaction::Structural`] — the customer resource. **The whole record is
//!   personal**: legal name, address, tax and foreign tax numbers, birth date,
//!   phone numbers, net worth, income, employer, gender, dependants, family
//!   member names, political affiliation. There is no safe subset to keep, and
//!   a field-by-field rule over 171 fields is a field-by-field opportunity to
//!   forget one. So the shape is kept and every leaf value is replaced with a
//!   placeholder of the same JSON type — a real date where a date was, a real
//!   decimal where a number was — which preserves what a serde test actually
//!   checks (field names, nesting, null versus present, the date and decimal
//!   parse paths) and preserves nothing about the person.

use std::fs;
use std::path::Path;

use serde_json::Value;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// The most elements kept in any nested array of a capped capture.
///
/// SPY's nested chain is 1.8 MB — every expiration and every strike. A fixture
/// is read by a person and checked into a repository, and the hundredth strike
/// pins down nothing the third did not. Capping keeps the shape (arrays stay
/// arrays, nesting stays nesting) and bounds the file.
const MAX_NESTED: usize = 3;

/// How much of a response may be written down.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Redaction {
    /// Public reference data. Stored as it arrived.
    None,
    /// Replaces the account number and nickname, keeps the structural flags.
    Account,
    /// Keeps the shape, replaces every leaf value.
    Structural,
    /// Public data, but bounded: nested arrays are truncated.
    Capped,
}

/// The value written where a string was.
const SENTINEL: &str = "REDACTED";
/// Written where a date was, so `NaiveDate` still parses.
const DATE: &str = "2020-01-01";
/// Written where a timestamp was, so `DateTime<FixedOffset>` still parses.
const TIMESTAMP: &str = "2020-01-01T00:00:00.000+00:00";

/// What to capture: the file stem, the path, and how much may be kept.
///
/// Listings are bounded. `/instruments/equities` alone holds 24,701 records and
/// a fixture is meant to be read by a person.
const CAPTURES: [(&str, &str, Redaction); 10] = [
    ("customer", "/customers/me", Redaction::Structural),
    ("accounts", "/customers/me/accounts", Redaction::Account),
    (
        "equities",
        "/instruments/equities?per-page=3",
        Redaction::None,
    ),
    (
        "cryptocurrencies",
        "/instruments/cryptocurrencies",
        Redaction::None,
    ),
    (
        "futures",
        "/instruments/futures?per-page=3",
        Redaction::None,
    ),
    (
        "future-products",
        "/instruments/future-products?per-page=2",
        Redaction::None,
    ),
    (
        "quantity-decimal-precisions",
        "/instruments/quantity-decimal-precisions",
        Redaction::None,
    ),
    (
        "nested-option-chain",
        "/option-chains/SPY/nested",
        Redaction::Capped,
    ),
    (
        "market-session-current",
        "/market-time/equities/sessions/current",
        Redaction::None,
    ),
    ("warrants", "/instruments/warrants", Redaction::None),
];

/// Queries tried for the instrument search, in order, until one returns rows.
///
/// `query=SPY` returns nothing in certification, and an empty listing exercises
/// no field of the type it is supposed to pin down.
const SEARCH_QUERIES: [&str; 5] = ["SPY", "A", "AAPL", "E", "SPX"];

/// Where captures go, relative to the workspace root.
const OUT_DIR: &str = "Doc/captures";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    println!("Capturing from {} (read-only)\n", config.environment());

    fs::create_dir_all(OUT_DIR)?;
    let mut written = Vec::new();
    let mut empty = Vec::new();

    for (stem, path, redaction) in CAPTURES {
        match capture(&tasty, stem, path, redaction).await {
            Ok(Some(count)) => {
                written.push(stem);
                println!("  {stem:<28} {count} record(s)  <- {path}");
            }
            Ok(None) => {
                empty.push(stem);
                println!("  {stem:<28} EMPTY, not written  <- {path}");
            }
            Err(error) => println!("  {stem:<28} failed: {error}"),
        }
    }

    // Search needs a query that actually matches something.
    let mut searched = false;
    for query in SEARCH_QUERIES {
        let path = format!("/instruments/search?query={query}");
        match capture(&tasty, "instrument-search", &path, Redaction::None).await {
            Ok(Some(count)) => {
                println!("  {:<28} {count} record(s)  <- {path}", "instrument-search");
                written.push("instrument-search");
                searched = true;
                break;
            }
            Ok(None) => continue,
            Err(error) => {
                println!("  {:<28} failed: {error}", "instrument-search");
                break;
            }
        }
    }
    if !searched {
        empty.push("instrument-search");
        println!(
            "  {:<28} EMPTY for every query tried, not written",
            "instrument-search"
        );
    }

    println!("\n{} capture(s) in {OUT_DIR}", written.len());
    if !empty.is_empty() {
        println!(
            "{} route(s) answered with nothing and were skipped: {}",
            empty.len(),
            empty.join(", ")
        );
        println!("An empty listing pins down no field, so no file is written for one.");
    }

    Ok(())
}

/// Fetches one route, redacts it, and writes it. `None` when it was empty.
async fn capture(
    tasty: &TastyTrade,
    stem: &str,
    path: &str,
    redaction: Redaction,
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    let body: Value = tasty.get(path).await?;

    // An empty listing decodes but pins down nothing, so it is not written:
    // a fixture that asserts a type's fields cannot be built from no records.
    let count = match body.pointer("/items").and_then(Value::as_array) {
        Some(items) if items.is_empty() => return Ok(None),
        Some(items) => items.len(),
        None => 1,
    };

    let mut redacted = body;
    apply(&mut redacted, redaction);

    let rendered = serde_json::to_string_pretty(&redacted)?;
    let file = Path::new(OUT_DIR).join(format!("{stem}.json"));
    fs::write(&file, format!("{rendered}\n"))?;

    Ok(Some(count))
}

/// Rewrites a captured value in place according to its tier.
fn apply(value: &mut Value, redaction: Redaction) {
    match redaction {
        Redaction::None => {}
        Redaction::Account => redact_account_fields(value),
        Redaction::Structural => blank_every_leaf(value),
        Redaction::Capped => cap_nested_arrays(value),
    }
}

/// Truncates every nested array so the capture stays a readable file.
///
/// Applied below the envelope's own `items`, which is already bounded by the
/// request's `per-page`. What this cuts is the second level and deeper — the
/// expirations of a chain and the strikes within them.
fn cap_nested_arrays(value: &mut Value) {
    match value {
        Value::Object(map) => map.values_mut().for_each(cap_nested_arrays),
        Value::Array(items) => {
            items.truncate(MAX_NESTED);
            items.iter_mut().for_each(cap_nested_arrays);
        }
        _ => {}
    }
}

/// Replaces the two fields on an account record that identify a person.
///
/// The rest of the record is flags — margin or cash, futures approved, closed —
/// which is what a serde test for this type exists to pin down.
fn redact_account_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                let identifying = key.ends_with("account-number")
                    || key == "account-number"
                    || key == "nickname"
                    || key == "external-id";
                if identifying && entry.is_string() {
                    *entry = Value::String(SENTINEL.to_string());
                } else {
                    redact_account_fields(entry);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_account_fields),
        _ => {}
    }
}

/// Keeps the shape and replaces every leaf value.
///
/// A string that parses as a date or a timestamp is replaced with another one,
/// so the fields whose serde path actually does work keep exercising it. `null`
/// survives as `null`: the difference between absent, null and present is the
/// property most of these tests are about, and flattening it would make the
/// fixture agree with a type that got optionality wrong.
fn blank_every_leaf(value: &mut Value) {
    match value {
        Value::Object(map) => map.values_mut().for_each(blank_every_leaf),
        Value::Array(items) => items.iter_mut().for_each(blank_every_leaf),
        Value::String(text) => {
            *value = Value::String(placeholder_for(text).to_string());
        }
        Value::Number(_) => *value = Value::Number(1.into()),
        Value::Bool(_) => *value = Value::Bool(false),
        Value::Null => {}
    }
}

/// The placeholder that keeps a string parseable as whatever it was.
fn placeholder_for(text: &str) -> &'static str {
    if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() {
        DATE
    } else if chrono::DateTime::parse_from_rfc3339(text).is_ok() {
        TIMESTAMP
    } else {
        SENTINEL
    }
}
