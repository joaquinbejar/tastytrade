//! SPAN risk rows for one exchange and day.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p account-data --bin span_rows
//! ```
//!
//! Both `date` and `exchange` are required by the venue, so both are arguments
//! rather than optional fields — a required query parameter should be
//! impossible to omit, not a runtime 400.

use chrono::Utc;
use tastytrade::prelude::SpanExchange;
use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

/// How many pages to walk before stopping regardless of what the venue says.
const MAX_PAGES: usize = 2;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;

    let mut page = PageRequest::first().with_per_page(5);
    for _ in 0..MAX_PAGES {
        let rows = tasty
            .span_rows(Utc::now().date_naive(), SpanExchange::Cme, &page)
            .await?;

        info!(
            "page {} of {}: {} row(s)",
            rows.pagination.page_offset,
            rows.pagination.total_pages,
            rows.len()
        );
        for row in &rows {
            // `row_data` is a fixed-width record in the exchange's own format.
            // Parsing it is a different job from talking to this API, so it
            // stays text.
            info!(
                "  {} #{}: {} byte(s) of row data",
                row.exchange.as_deref().unwrap_or("-"),
                row.row_index.unwrap_or_default(),
                row.row_data.as_deref().map(str::len).unwrap_or(0)
            );
        }

        if !rows.has_more() {
            break;
        }
        page = page.next_page();
    }

    Ok(())
}
