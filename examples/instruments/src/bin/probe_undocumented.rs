//! Asks the venue whether two release-note-only routes still exist.
//!
//! `GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
//! are named in the 2025-07-15 release note as newly paginated, and neither
//! appears in the Instruments OpenAPI document published under that same date.
//! Release notes are not a client contract, and a spec that omits a route is
//! not proof the route is gone — see `Doc/API_Coverage_Status.md` for why both
//! statements are true here at once.
//!
//! Settling it needs the venue, so this is the reproducible half: one read-only
//! GET per route, reporting what came back.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin probe_undocumented
//! ```
//!
//! Prints **shape, never content**: the status, which envelope decoded, and
//! the field names of the first item. Field names are the contract; the values
//! are somebody's market data and are not what this question is about.

use serde_json::Value;
use tastytrade::api::base::{Items, Paginated};
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// The routes in question, plus the ones that make the answer interpretable.
///
/// The controls matter more than they look. `/instruments/equities` is in the
/// current spec *and* in the release note, so it shows what a supported
/// paginated listing answers. `/instruments/equity-options` is in the release
/// note and **absent from the current spec**, yet this crate calls it — so if
/// it answers, absence from the spec means nothing on its own, and the two
/// routes under investigation cannot be declared retired on that basis.
const ROUTES: [(&str, &str); 4] = [
    ("/instruments/equity-deliverables", "under investigation"),
    ("/instruments/future-spreads", "under investigation"),
    ("/instruments/equities", "control: in the current spec"),
    (
        "/instruments/equity-options",
        "control: absent from the current spec, implemented here",
    ),
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
    println!("Probing {} (read-only GETs only)\n", config.environment());

    for (path, note) in ROUTES {
        println!("{path}  — {note}");
        println!("  {}", probe(&tasty, path).await);
        println!();
    }

    println!("Record the result in Doc/API_Coverage_Status.md with today's date,");
    println!("and only then design types for anything that answered.");

    Ok(())
}

/// One route, described by what it answered.
///
/// Tries the envelopes in decreasing order of structure. A paginated listing
/// also decodes as `Items`, so the order is what distinguishes them — asking
/// for `Paginated` first is the only way to learn whether the pagination block
/// the release note promises is actually there.
async fn probe(tasty: &TastyTrade, path: &str) -> String {
    // The listing endpoints cap at 1000 by default; one item is enough to read
    // a shape, and asking for one keeps a probe from pulling 25,000 rows.
    let query = [("per-page", "1")];

    if let Ok(page) = tasty
        .get_with_query::<Items<Value>, Paginated<Value>, _>(path, &query)
        .await
    {
        return format!(
            "200, paginated: {} item(s) on this page of {} total; first item fields: {}",
            page.items.len(),
            page.pagination.total_items,
            field_names(page.items.first())
        );
    }

    if let Ok(items) = tasty
        .get_with_query::<Items<Value>, Items<Value>, _>(path, &query)
        .await
    {
        return format!(
            "200, items envelope with no pagination block: {} item(s); first item fields: {}",
            items.items.len(),
            field_names(items.items.first())
        );
    }

    match tasty.get_with_query::<Value, Value, _>(path, &query).await {
        Ok(value) => format!("200, single object; fields: {}", field_names(Some(&value))),
        // The status is the answer. 404 means the route is gone, 401/403 means
        // it exists and this application is not entitled to it, and those are
        // very different conclusions to write into a coverage table.
        Err(TastyTradeError::Request { context, .. }) => match context.status {
            Some(404) => "404 — the venue does not route this path".to_string(),
            Some(status) => format!(
                "{status} — the route exists; this is an authorisation or \
                 parameter answer, not an absence"
            ),
            None => "no response reached the venue".to_string(),
        },
        Err(other) => format!("failed before a status was known: {other}"),
    }
}

/// The keys of a JSON object, sorted. Names only.
fn field_names(value: Option<&Value>) -> String {
    match value.and_then(Value::as_object) {
        Some(map) => {
            let mut names: Vec<&str> = map.keys().map(String::as_str).collect();
            names.sort_unstable();
            names.join(", ")
        }
        None => "(not an object)".to_string(),
    }
}
