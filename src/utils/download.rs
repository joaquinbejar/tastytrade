/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/

//! Bulk download of option symbols.
//!
//! Split into stages that can be reasoned about separately: discovery finds
//! the underlyings, retrieval fetches their chains, and transformation turns a
//! chain into symbol entries. Only the retrieval stage does I/O, only the
//! transformation stage is pure, and the report says which parts of the answer
//! are missing rather than leaving that in the logs.

use crate::api::query::PageRequest;
use crate::prelude::{SymbolEntry, TastyTradeConfig};
use crate::types::instrument::{FuturesNestedOptionChain, NestedOptionChain};
use crate::types::instrument_filter::ActiveEquityFilter;
use crate::utils::parse::expiration_instant;
use crate::{InstrumentType, TastyResult, TastyTrade, TastyTradeError};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use futures_util::stream;
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// The venue this crate labels downloaded symbols with.
const EXCHANGE: &str = "TASTYTRADE";

/// Items per page when walking a listing.
///
/// The value `list_active_equities` used to hard-code before the page size
/// became the caller's decision.
const EQUITY_PAGE_SIZE: u32 = 1000;

/// Future products that do not carry option chains.
///
/// Asking for them costs a round trip and answers nothing, so they are skipped
/// rather than retried. Interest-rate futures, mostly.
const PRODUCTS_WITHOUT_OPTIONS: &[&str] = &["GE", "ZQ", "ZT", "ZF", "ZN", "ZB", "UB"];

/// Limits and concurrency for a download.
///
/// Every knob that used to be an undocumented environment variable or a
/// literal buried in a loop. `MAX_EQUITIES` and `MAX_FUTURE_PRODUCTS` were
/// read from the environment by a function nothing documented, so a caller had
/// no way to discover them and a library had no business reading them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadLimits {
    /// How many pages of active equities to walk before stopping.
    pub max_equity_pages: usize,
    /// How many equities to fetch chains for.
    pub max_equities: usize,
    /// How many future products to fetch chains for.
    pub max_future_products: usize,
    /// How many chain requests may be in flight at once.
    ///
    /// The old code was strictly sequential, which is slow, but unbounded
    /// concurrency against a broker is a good way to be rate limited or
    /// blocked.
    pub concurrency: usize,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self {
            max_equity_pages: 5,
            max_equities: 100,
            max_future_products: 50,
            concurrency: 8,
        }
    }
}

/// One underlying whose chain could not be retrieved.
///
/// Named so a caller can retry exactly what failed instead of the whole run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFailure {
    /// The underlying symbol or product code that failed.
    pub underlying: String,
    /// Why, as text. Never contains a credential or an account.
    pub reason: String,
    /// Whether asking again could plausibly work.
    ///
    /// Carried rather than left for the caller to infer from `reason`:
    /// deciding what to retry by matching on prose is how the old code
    /// classified failures, and it was wrong the moment the venue reworded
    /// anything.
    pub retryable: bool,
}

impl DownloadFailure {
    fn from_error(underlying: String, error: &TastyTradeError) -> Self {
        Self {
            underlying,
            reason: error.to_string(),
            retryable: error.is_retryable(),
        }
    }
}

/// How complete a download is.
///
/// The old function logged failures and returned a plain `Vec`, so a caller
/// could not tell a complete answer from one missing half its underlyings —
/// and a short list looks exactly like a quiet market.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadOutcome {
    /// Every underlying that was asked for answered.
    Complete,
    /// Some underlyings failed. The symbols present are still usable.
    Partial {
        /// What failed and why.
        failures: Vec<DownloadFailure>,
    },
}

/// The symbols and how much of the picture they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadReport {
    /// The symbols, deduplicated and sorted.
    pub symbols: Vec<SymbolEntry>,
    /// Whether anything is missing.
    pub outcome: DownloadOutcome,
    /// How many underlyings were asked about.
    pub underlyings_requested: usize,
}

impl DownloadReport {
    /// Whether every underlying answered.
    pub fn is_complete(&self) -> bool {
        matches!(self.outcome, DownloadOutcome::Complete)
    }

    /// What failed, empty on a complete run.
    pub fn failures(&self) -> &[DownloadFailure] {
        match &self.outcome {
            DownloadOutcome::Complete => &[],
            DownloadOutcome::Partial { failures } => failures,
        }
    }
}

/// Downloads every equity-option and future-option symbol, with defaults.
///
/// # Errors
///
/// Fails only when the download could not start: missing credentials, a
/// refused login, or no underlyings discovered at all. An underlying whose
/// chain fails is recorded in the report rather than failing the run.
pub async fn download_options_report() -> TastyResult<DownloadReport> {
    let config = TastyTradeConfig::new();
    download_options_symbols_with(&config, &DownloadLimits::default()).await
}

/// Downloads every option symbol, discarding the report.
///
/// # Errors
///
/// As [`download_options_report`].
#[deprecated(
    since = "0.4.0",
    note = "use download_options_report: this signature cannot tell a complete \
            answer from one missing half its underlyings"
)]
pub async fn download_options_symbols() -> Result<Vec<SymbolEntry>, Box<dyn std::error::Error>> {
    // Kept signature-compatible on purpose. The report is the improvement, but
    // a caller who only wants the symbols should not have to be broken to keep
    // getting them.
    Ok(download_options_report().await?.symbols)
}

/// Downloads every equity-option and future-option symbol.
///
/// The configuration is taken rather than read from the environment, so a
/// caller decides which account and which environment this runs against.
pub async fn download_options_symbols_with(
    config: &TastyTradeConfig,
    limits: &DownloadLimits,
) -> TastyResult<DownloadReport> {
    // connect already refuses missing credentials without a network call, and
    // says which variables to set.
    let tasty = TastyTrade::connect(config).await?;
    let now = Utc::now();

    // The two sources are independent, so one failing must not discard the
    // other's work or stop it from running at all.
    let mut failures = Vec::new();

    let equities = match discover_equities(&tasty, limits).await {
        Ok(found) => found,
        Err(e) => {
            failures.push(DownloadFailure::from_error(
                "equity discovery".to_string(),
                &e,
            ));
            Vec::new()
        }
    };
    let products = match discover_future_products(&tasty, limits).await {
        Ok(found) => found,
        Err(e) => {
            failures.push(DownloadFailure::from_error(
                "future product discovery".to_string(),
                &e,
            ));
            Vec::new()
        }
    };

    // Both failing is not a partial answer, it is no answer.
    if equities.is_empty() && products.is_empty() {
        return Err(TastyTradeError::Unknown(
            "no equity or future underlyings were discovered; check connectivity and that the \
             account can see instruments"
                .to_string(),
        ));
    }

    let requested = equities.len() + products.len();
    info!(
        "Downloading option chains for {} equities and {} future products",
        equities.len(),
        products.len()
    );

    let mut symbols = Vec::new();

    let (equity_symbols, equity_failures) =
        fetch_equity_chains(&tasty, &equities, limits, now).await;
    symbols.extend(equity_symbols);
    failures.extend(equity_failures);

    let (future_symbols, future_failures) =
        fetch_future_chains(&tasty, &products, limits, now).await;
    symbols.extend(future_symbols);
    failures.extend(future_failures);

    // Deduplicated by identity, which for a SymbolEntry is symbol plus epic,
    // then sorted. Concurrent retrieval means arrival order is not stable, and
    // a bulk download that produces a different file every run is one nobody
    // can diff.
    let unique: HashSet<SymbolEntry> = symbols.into_iter().collect();
    let mut symbols: Vec<SymbolEntry> = unique.into_iter().collect();
    symbols.sort_unstable_by(|a, b| {
        a.symbol
            .cmp(&b.symbol)
            .then_with(|| a.epic.cmp(&b.epic))
            .then_with(|| a.expiry.cmp(&b.expiry))
    });

    // Sorted for the same reason the symbols are: buffer_unordered decides the
    // order failures arrive in, and a report that differs between identical
    // runs is not the deterministic report this claims to be.
    failures.sort_unstable_by(|a, b| {
        a.underlying
            .cmp(&b.underlying)
            .then_with(|| a.reason.cmp(&b.reason))
    });

    if failures.is_empty() {
        info!("Downloaded {} unique symbols", symbols.len());
    } else {
        warn!(
            "Downloaded {} unique symbols; {} of {} underlyings failed",
            symbols.len(),
            failures.len(),
            requested
        );
    }

    Ok(DownloadReport {
        symbols,
        outcome: if failures.is_empty() {
            DownloadOutcome::Complete
        } else {
            DownloadOutcome::Partial { failures }
        },
        underlyings_requested: requested,
    })
}

/// Walks the active-equities pages up to the configured limit.
async fn discover_equities(
    tasty: &TastyTrade,
    limits: &DownloadLimits,
) -> TastyResult<Vec<crate::types::instrument::EquityInstrument>> {
    let mut found = Vec::new();

    for page in 0..limits.max_equity_pages {
        // The page size the walk has always used. Explicit now rather than
        // buried in the endpoint method, because it is a decision about this
        // downloader and not about the endpoint.
        let request = PageRequest::new()
            .with_per_page(EQUITY_PAGE_SIZE)
            .with_page_offset(u32::try_from(page).unwrap_or(u32::MAX));
        let paginated = tasty
            .list_active_equities(&ActiveEquityFilter::new().with_page(request))
            .await?;
        let pagination = &paginated.pagination;
        debug!(
            "active equities page {}/{}: {} items",
            pagination.page_offset, pagination.total_pages, pagination.current_item_count
        );

        let last_page = pagination.page_offset + 1 >= pagination.total_pages;
        found.extend(paginated.items);

        if last_page {
            break;
        }
    }

    if found.len() > limits.max_equities {
        info!(
            "Limiting to {} of {} equities",
            limits.max_equities,
            found.len()
        );
        found.truncate(limits.max_equities);
    }

    Ok(found)
}

/// Lists future products worth asking about.
async fn discover_future_products(
    tasty: &TastyTrade,
    limits: &DownloadLimits,
) -> TastyResult<Vec<crate::types::instrument::FutureProduct>> {
    // Future products paginate now, so this walks pages instead of assuming
    // one response is the whole catalogue. Bounded by the same page budget as
    // equities: a downloader that walks until the venue says stop is a
    // downloader that hangs when the venue is wrong about that.
    let mut products: Vec<crate::types::instrument::FutureProduct> = Vec::new();

    for page in 0..limits.max_equity_pages {
        let request = PageRequest::new()
            .with_per_page(EQUITY_PAGE_SIZE)
            .with_page_offset(u32::try_from(page).unwrap_or(u32::MAX));
        let paginated = tasty.list_future_products(&request).await?;
        let pagination = &paginated.pagination;
        debug!(
            "future products page {}/{}: {} items",
            pagination.page_offset, pagination.total_pages, pagination.current_item_count
        );

        let last_page = pagination.page_offset + 1 >= pagination.total_pages;
        products.extend(
            paginated
                .items
                .into_iter()
                .filter(|product| !PRODUCTS_WITHOUT_OPTIONS.contains(&product.code.as_str())),
        );

        if last_page {
            break;
        }
    }

    if products.len() > limits.max_future_products {
        info!(
            "Limiting to {} of {} future products",
            limits.max_future_products,
            products.len()
        );
        products.truncate(limits.max_future_products);
    }

    Ok(products)
}

/// Fetches equity chains with bounded concurrency.
async fn fetch_equity_chains(
    tasty: &TastyTrade,
    equities: &[crate::types::instrument::EquityInstrument],
    limits: &DownloadLimits,
    last_update: DateTime<Utc>,
) -> (Vec<SymbolEntry>, Vec<DownloadFailure>) {
    let results = stream::iter(equities.iter().map(|equity| async move {
        let symbol = equity.symbol.clone();
        (
            symbol.0.clone(),
            tasty.list_nested_option_chains(symbol).await,
        )
    }))
    .buffer_unordered(limits.concurrency.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut symbols = Vec::new();
    let mut failures = Vec::new();

    for (underlying, result) in results {
        match result {
            Ok(chains) => {
                for chain in &chains {
                    symbols.extend(equity_chain_to_symbols(chain, last_update));
                }
            }
            Err(e) => failures.push(DownloadFailure::from_error(underlying, &e)),
        }
    }

    (symbols, failures)
}

/// Fetches future-option chains with bounded concurrency.
async fn fetch_future_chains(
    tasty: &TastyTrade,
    products: &[crate::types::instrument::FutureProduct],
    limits: &DownloadLimits,
    last_update: DateTime<Utc>,
) -> (Vec<SymbolEntry>, Vec<DownloadFailure>) {
    let results = stream::iter(products.iter().map(|product| async move {
        (
            product.code.clone(),
            tasty.list_nested_futures_option_chains(&product.code).await,
        )
    }))
    .buffer_unordered(limits.concurrency.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut symbols = Vec::new();
    let mut failures = Vec::new();

    for (underlying, result) in results {
        match result {
            Ok(chains) => {
                for chain in &chains {
                    symbols.extend(futures_chain_to_symbols(chain, last_update));
                }
            }
            Err(e) => failures.push(DownloadFailure::from_error(underlying, &e)),
        }
    }

    (symbols, failures)
}

/// Turns one equity chain into symbol entries.
///
/// Pure: no I/O, no logging, no clock. This is the part worth testing, and it
/// used to be four levels deep inside a function that also did all three.
fn equity_chain_to_symbols(
    chain: &NestedOptionChain,
    last_update: DateTime<Utc>,
) -> Vec<SymbolEntry> {
    let mut symbols = Vec::new();

    for expiration in &chain.expirations {
        let expiry = expiration_instant(expiration.expiration_date);

        for strike in &expiration.strikes {
            for (side, symbol) in [("Call", &strike.call), ("Put", &strike.put)] {
                symbols.push(SymbolEntry {
                    symbol: symbol.0.clone(),
                    epic: symbol.0.clone(),
                    name: format!(
                        "{} {} ${} {}",
                        chain.underlying_symbol.0,
                        side,
                        strike.strike_price,
                        expiration.expiration_date
                    ),
                    instrument_type: InstrumentType::EquityOption,
                    exchange: EXCHANGE.to_string(),
                    expiry,
                    last_update,
                });
            }
        }
    }

    symbols
}

/// Turns one nested futures chain into symbol entries. Pure, as above.
fn futures_chain_to_symbols(
    chain: &FuturesNestedOptionChain,
    last_update: DateTime<Utc>,
) -> Vec<SymbolEntry> {
    let mut symbols = Vec::new();

    for option_chain in &chain.option_chains {
        for expiration in &option_chain.expirations {
            let expiry = expiration_instant(expiration.expiration_date);

            for strike in &expiration.strikes {
                for (side, symbol) in [("Call", &strike.call), ("Put", &strike.put)] {
                    symbols.push(SymbolEntry {
                        symbol: symbol.clone(),
                        epic: symbol.clone(),
                        name: format!(
                            "{} Future {} ${} {}",
                            option_chain.underlying_symbol,
                            side,
                            strike.strike_price,
                            expiration.expiration_date
                        ),
                        instrument_type: InstrumentType::FutureOption,
                        exchange: EXCHANGE.to_string(),
                        expiry,
                        last_update,
                    });
                }
            }
        }
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).expect("a valid timestamp")
    }

    #[test]
    fn a_complete_report_has_no_failures() {
        let report = DownloadReport {
            symbols: Vec::new(),
            outcome: DownloadOutcome::Complete,
            underlyings_requested: 3,
        };

        assert!(report.is_complete());
        assert!(report.failures().is_empty());
    }

    /// The old function logged failures and returned a plain Vec, so a caller
    /// could not tell a complete answer from one missing half its underlyings.
    /// A short list looks exactly like a quiet market.
    #[test]
    fn a_partial_report_names_what_is_missing() {
        let report = DownloadReport {
            symbols: Vec::new(),
            outcome: DownloadOutcome::Partial {
                failures: vec![DownloadFailure {
                    underlying: "AAPL".to_string(),
                    reason: "HTTP 503".to_string(),
                    retryable: true,
                }],
            },
            underlyings_requested: 2,
        };

        assert!(!report.is_complete());
        assert_eq!(report.failures().len(), 1);
        assert_eq!(report.failures()[0].underlying, "AAPL");
        assert!(
            report.failures()[0].retryable,
            "a caller must not have to parse the reason to decide what to retry"
        );
    }

    /// The retry verdict comes from the error's own classification, not from
    /// matching on its prose, which is how the old code got it wrong.
    #[test]
    fn a_failure_carries_the_errors_own_retry_verdict() {
        let transient = DownloadFailure::from_error(
            "AAPL".to_string(),
            &TastyTradeError::Connection("refused".to_string()),
        );
        let fatal = DownloadFailure::from_error(
            "MSFT".to_string(),
            &TastyTradeError::Auth("rejected".to_string()),
        );

        assert!(transient.retryable);
        assert!(!fatal.retryable, "a rejected credential does not improve");
    }

    /// buffer_unordered decides the order failures arrive in, so a report that
    /// differs between identical runs would not be the deterministic report
    /// this claims to be.
    #[test]
    fn failures_are_ordered_deterministically() {
        let mut failures = [
            DownloadFailure {
                underlying: "MSFT".to_string(),
                reason: "b".to_string(),
                retryable: true,
            },
            DownloadFailure {
                underlying: "AAPL".to_string(),
                reason: "a".to_string(),
                retryable: false,
            },
        ];
        failures.sort_unstable_by(|a, b| {
            a.underlying
                .cmp(&b.underlying)
                .then_with(|| a.reason.cmp(&b.reason))
        });

        assert_eq!(failures[0].underlying, "AAPL");
    }

    /// Interest-rate futures carry no option chains, so asking costs a round
    /// trip and answers nothing.
    #[test]
    fn products_without_options_are_known_by_code() {
        for code in ["GE", "ZN", "UB"] {
            assert!(
                PRODUCTS_WITHOUT_OPTIONS.contains(&code),
                "{code} should be skipped"
            );
        }
        assert!(!PRODUCTS_WITHOUT_OPTIONS.contains(&"ES"));
    }

    /// Limits are a documented type now rather than environment variables a
    /// caller had no way to discover.
    #[test]
    fn the_default_limits_are_bounded_on_every_axis() {
        let limits = DownloadLimits::default();

        assert!(limits.max_equity_pages > 0);
        assert!(limits.max_equities > 0);
        assert!(limits.max_future_products > 0);
        assert!(
            limits.concurrency > 1,
            "the point of the refactor is that it is not sequential"
        );
    }

    /// Concurrent retrieval means arrival order is not stable, so a bulk
    /// download that produced a different file every run would be one nobody
    /// can diff.
    #[test]
    fn identical_symbols_from_different_sources_collapse_to_one() {
        let entry = |symbol: &str, expiry| SymbolEntry {
            symbol: symbol.to_string(),
            epic: symbol.to_string(),
            name: format!("{symbol} option"),
            instrument_type: InstrumentType::EquityOption,
            exchange: EXCHANGE.to_string(),
            expiry,
            last_update: at(0),
        };

        let unique: HashSet<SymbolEntry> = vec![
            entry("AAPL 250919C00100000", at(10)),
            entry("AAPL 250919C00100000", at(10)),
            entry("MSFT 250919P00300000", at(20)),
        ]
        .into_iter()
        .collect();

        assert_eq!(unique.len(), 2, "identity is symbol plus epic");
    }
}
