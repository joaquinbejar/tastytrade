//! Walks a listing past its first page, filtered.
//!
//! The instrument listings return one page at a time, and until they returned
//! `Paginated<T>` there was no way to ask for the second one — the pagination
//! block arrived and was discarded. This is what reading a whole listing looks
//! like now.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin page_through_equities
//! ```
//!
//! Bounded on purpose: it stops after [`MAX_PAGES`] whatever the venue says.
//! A loop that trusts `total_pages` is a loop that runs until the venue is
//! wrong about it.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// How many pages to walk before stopping regardless.
const MAX_PAGES: usize = 3;

/// Items per page. Small so the example shows several pages of a listing that
/// really has tens of thousands of rows.
const PER_PAGE: u32 = 25;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    // Every parameter the endpoint documents, in one filter. Leaving one unset
    // omits it, so the venue's own default is what answers.
    let mut filter = EquityFilter::new()
        .with_is_etf(true)
        .with_lendability(Lendability::EasyToBorrow)
        .with_page(PageRequest::first().with_per_page(PER_PAGE));

    let mut seen = 0usize;

    for _ in 0..MAX_PAGES {
        let page = tasty.list_equities(&filter).await?;

        info!(
            "page {} of {}: {} item(s), {} in the listing",
            page.pagination.page_offset,
            page.pagination.total_pages,
            page.len(),
            page.pagination.total_items
        );

        for equity in &page {
            info!(
                "  {} — {} (ETF: {}, lendability: {:?})",
                equity.symbol.0, equity.description, equity.is_etf, equity.lendability
            );
        }
        seen += page.len();

        // `has_more` rather than a hand-rolled comparison: page offsets count
        // from zero, so the last page is `total_pages - 1` and that is exactly
        // the off-by-one worth not writing twice.
        if !page.has_more() {
            info!("that was the last page");
            break;
        }

        filter = filter.with_page(page_after(&page, PER_PAGE));
    }

    info!("{seen} equity(ies) read across at most {MAX_PAGES} page(s)");

    // The same shape works for futures, where the filter that could not be
    // expressed before was several product codes at once.
    let futures = tasty
        .list_futures(
            &FutureFilter::for_product_codes(&["ES", "6A"])
                .with_page(PageRequest::first().with_per_page(PER_PAGE)),
        )
        .await?;
    info!(
        "{} future(s) across the ES and 6A products on page 0 of {}",
        futures.len(),
        futures.pagination.total_pages
    );

    Ok(())
}

/// The page after `page`, keeping the size.
///
/// Derived from where the venue says the page landed rather than from a local
/// counter, so a venue that answers a different page than the one asked for
/// cannot send this into a loop.
fn page_after<T>(page: &Paginated<T>, per_page: u32) -> PageRequest {
    PageRequest::new()
        .with_per_page(per_page)
        .with_page_offset(u32::try_from(page.pagination.page_offset).unwrap_or(u32::MAX))
        .next_page()
}
