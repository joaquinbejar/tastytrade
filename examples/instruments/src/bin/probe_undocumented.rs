//! Asks the venue whether two release-note-only routes still exist.
//!
//! `GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
//! are named in the 2025-07-15 release note as newly paginated, and neither
//! appears in the Instruments OpenAPI document published under that same date.
//! Release notes are not a client contract, and a spec that omits a route is
//! not proof the route is gone — see `Doc/API_Coverage_Status.md` for why both
//! statements are true here at once.
//!
//! Settling it needs the venue, so this is the reproducible half: read-only
//! GETs per route, reporting what came back and whether the pagination the
//! release note promises is actually honoured.
//!
//! Run it against **both** hosts before concluding anything. A route
//! certification does not serve is not a route production has retired, and the
//! coverage table has to say which question was answered.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true  cargo run -p instruments --bin probe_undocumented
//! TASTYTRADE_USE_DEMO=false cargo run -p instruments --bin probe_undocumented
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
/// The controls matter more than they look, and the negative one matters most.
///
/// `/instruments/equities` is in the current spec *and* in the release note, so
/// it shows what a supported paginated listing answers.
/// `/instruments/equity-options` is in the release note and **absent from the
/// current spec**, yet this crate calls it — so if it answers, absence from the
/// spec means nothing on its own.
///
/// The last one cannot exist. It is what makes a refusal readable: if a path
/// nobody ever routed comes back with the same status as the two under
/// investigation, then that status is what this deployment says to *any*
/// unrecognised path and carries no information about these two. Without it a
/// `403` reads as "exists but forbidden", which is a conclusion the evidence
/// does not support.
const ROUTES: [(&str, &str); 5] = [
    ("/instruments/equity-deliverables", "under investigation"),
    ("/instruments/future-spreads", "under investigation"),
    (
        "/instruments/equities",
        "positive control: in the current spec",
    ),
    (
        "/instruments/equity-options",
        "control: absent from the current spec, implemented here",
    ),
    (
        "/instruments/there-is-no-such-route",
        "negative control: certainly does not exist",
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

    println!("Run this against the other host too before concluding: a route");
    println!("certification does not serve is not a route production retired.");
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
            "200, paginated: {} item(s) on this page of {} total; {}; first item fields: {}",
            page.items.len(),
            page.pagination.total_items,
            paging_honoured(tasty, path).await,
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
        // The status is the answer, read against the negative control below.
        // The broker's own error code survives sanitisation and is the most
        // specific thing available; its prose does not and is not printed.
        Err(TastyTradeError::Request { context, api }) => {
            let code = api
                .as_ref()
                .and_then(|error| error.code.as_deref())
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default();
            match context.status {
                Some(404) => format!("404{code} — the venue does not route this path"),
                Some(status) => format!(
                    "{status}{code} — compare with the negative control before \
                     reading this as the route existing"
                ),
                None => "no response reached the venue".to_string(),
            }
        }
        // A local refusal never reached the venue, so it says nothing about
        // the route — reporting it as an answer would be the probe lying.
        Err(TastyTradeError::Precondition(why)) => {
            format!("refused locally, nothing was sent: {why}")
        }
        Err(other) => format!("failed before a status was known: {other}"),
    }
}

/// Whether the route honours the page parameters, or merely tolerates them.
///
/// Pagination is the **only** thing the release note claims about these two
/// routes, so it is the one part of the contract there is anything to check.
/// Accepting `page-offset` without acting on it is a real outcome and a
/// different one from supporting it: a caller that pages through a listing
/// which ignores the offset reads page one forever.
async fn paging_honoured(tasty: &TastyTrade, path: &str) -> String {
    let first = [("per-page", "1"), ("page-offset", "0")];
    let second = [("per-page", "1"), ("page-offset", "1")];

    let one = tasty
        .get_with_query::<Items<Value>, Paginated<Value>, _>(path, &first)
        .await;
    let two = tasty
        .get_with_query::<Items<Value>, Paginated<Value>, _>(path, &second)
        .await;

    let (one, two) = match (one, two) {
        (Ok(one), Ok(two)) => (one, two),
        (_, Err(TastyTradeError::Request { context, .. })) => {
            return match context.status {
                Some(status) => format!("page-offset rejected with {status}"),
                None => "page-offset: no response".to_string(),
            };
        }
        (Err(error), _) | (_, Err(error)) => return format!("page-offset: {error}"),
    };

    // Echoing the request back is not evidence. A handler can populate
    // `page-offset` from what it was asked for and still serve page zero,
    // which is exactly the tolerated-but-ignored behaviour this is looking
    // for, so the echo is checked and then not believed on its own.
    if two.pagination.page_offset != 1 {
        return format!(
            "page-offset NOT honoured (asked for offset 1, the response says offset {})",
            two.pagination.page_offset
        );
    }

    if two.pagination.total_items <= 1 {
        return "page-offset: inconclusive, the listing has at most one item".to_string();
    }

    // Identity, never content: whether the two pages are the same record, not
    // what either record says.
    match (one.items.first(), two.items.first()) {
        (Some(a), Some(b)) if a == b => {
            "page-offset NOT honoured (offset 1 returned the same record as offset 0)".to_string()
        }
        (Some(_), Some(_)) => {
            "page-offset honoured (offset 1 returned a different record from offset 0)".to_string()
        }
        _ => "page-offset: inconclusive, a page came back empty".to_string(),
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
