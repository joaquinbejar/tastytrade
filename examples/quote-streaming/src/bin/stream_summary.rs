//! Streams the trading day's open, extremes and previous close.
//!
//! A `Summary` arrives once per symbol when the subscription is made and then
//! only when the day's figures change, so a run outside market hours prints a
//! line or two and stops.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_summary
//! ```

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(EventKind::Summary, "AAPL,SPY", 10, |symbol, data| {
        if let EventData::Summary(summary) = data {
            println!(
                "{symbol}: open {} high {} low {} close {} ({})",
                summary.day_open_price,
                summary.day_high_price,
                summary.day_low_price,
                summary.day_close_price,
                summary.day_close_price_type
            );
            println!(
                "  previous close {} ({}), previous volume {}",
                summary.prev_day_close_price,
                summary.prev_day_close_price_type,
                summary.prev_day_volume
            );
        }
    })
    .await
}
