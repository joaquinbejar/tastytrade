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
//! - [`Redaction::Structural`] — the customer resource **and the account
//!   listing**. The customer record is obviously personal: legal name, address,
//!   tax numbers, birth date, net worth, employer, political affiliation. The
//!   account listing is less obviously so and is the same problem — when it was
//!   created and opened, its investment objective, margin or cash, options
//!   level, futures approval. Those are the owner's financial profile, this
//!   file is packaged and published, and publication does not come back.
//!
//!   Replacing only the direct identifiers is a judgement call per field, and
//!   a field-by-field rule is a field-by-field chance to miss one. So the shape
//!   is kept and **every leaf value** is replaced with a placeholder of the
//!   same JSON type — a real date where a date was — which preserves what a
//!   serde test actually checks (field names, nesting, null versus present, the
//!   date and decimal parse paths) and preserves nothing about anybody.
//!
//! Nothing is written until [`assert_blanked`] confirms no original leaf
//! survived, so the tier cannot quietly fail open.

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
    ("accounts", "/customers/me/accounts", Redaction::Structural),
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
    let mut failed = Vec::new();

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
            Err(error) => {
                failed.push(stem);
                println!("  {stem:<28} failed: {error}");
            }
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
                failed.push("instrument-search");
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

    // A run that printed a failure and exited zero is a run that looks like it
    // refreshed everything. The stale file left behind is worse than no file,
    // because the tests keep passing against it.
    if !failed.is_empty() {
        return Err(format!(
            "{} route(s) failed: {}. Earlier captures for them were removed rather \
             than left stale.",
            failed.len(),
            failed.join(", ")
        )
        .into());
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
    // Removed first, so a failure below cannot leave the previous run's file
    // sitting there looking current.
    let file = Path::new(OUT_DIR).join(format!("{stem}.json"));
    if file.exists() {
        fs::remove_file(&file)?;
    }

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

    // Fail closed. A blanking that silently missed a leaf would publish it, and
    // publication does not come back, so the file is not written unless this
    // holds.
    if redaction == Redaction::Structural {
        assert_blanked(&redacted, stem)?;
    }

    let rendered = serde_json::to_string_pretty(&redacted)?;
    fs::write(&file, format!("{rendered}\n"))?;

    Ok(Some(count))
}

/// Fails unless every leaf is a placeholder.
///
/// The check the redaction cannot perform on itself: it walks the finished
/// value and refuses anything that is not one of the constants above, `null`,
/// `false`, or the numeric placeholder. `null` survives on purpose — the
/// difference between absent, null and present is the property most of these
/// fixtures exist to pin down.
fn assert_blanked(value: &Value, stem: &str) -> Result<(), Box<dyn std::error::Error>> {
    fn survivor(node: &Value) -> Option<String> {
        match node {
            Value::Object(map) => map.values().find_map(survivor),
            Value::Array(items) => items.iter().find_map(survivor),
            Value::String(text) => {
                (text != SENTINEL && text != DATE && text != TIMESTAMP).then(|| text.clone())
            }
            Value::Number(number) => (number.as_i64() != Some(1)).then(|| number.to_string()),
            Value::Bool(flag) => (*flag).then(|| "true".to_string()),
            Value::Null => None,
        }
    }

    match survivor(value) {
        // The value itself is not printed: it is the thing being kept out.
        Some(_) => Err(format!("{stem}: a leaf survived the blanking, refusing to write").into()),
        None => Ok(()),
    }
}

/// Rewrites a captured value in place according to its tier.
fn apply(value: &mut Value, redaction: Redaction) {
    match redaction {
        Redaction::None => {}
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
    // The envelope's own `items` is already bounded by the request's
    // `per-page`, and truncating it here would silently drop records the
    // caller asked for. Only what is nested inside a record is capped.
    match value.get_mut("items").and_then(Value::as_array_mut) {
        Some(records) => records.iter_mut().for_each(cap_below_root),
        None => cap_below_root(value),
    }
}

/// Truncates every array from here down.
fn cap_below_root(value: &mut Value) {
    match value {
        Value::Object(map) => map.values_mut().for_each(cap_below_root),
        Value::Array(items) => {
            items.truncate(MAX_NESTED);
            items.iter_mut().for_each(cap_below_root);
        }
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
