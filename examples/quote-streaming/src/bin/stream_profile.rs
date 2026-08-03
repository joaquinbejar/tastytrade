//! Streams instrument metadata: description, trading status, fundamentals.
//!
//! A `Profile` arrives once per symbol on subscription and then only when
//! something about the instrument changes — a halt, a limit change, a new
//! dividend. A short run normally prints one line per symbol and stops.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_profile
//! ```

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(EventKind::Profile, "AAPL,SPY", 10, |symbol, data| {
        if let EventData::Profile(profile) = data {
            println!("{symbol}: {}", profile.description);
            println!(
                "  status {} ({}), limits {}–{}",
                profile.trading_status,
                if profile.status_reason.is_empty() {
                    "no reason given"
                } else {
                    &profile.status_reason
                },
                profile.low_limit_price,
                profile.high_limit_price
            );
            println!(
                "  52 week {}–{}, beta {}, EPS {}",
                profile.low_52_week_price,
                profile.high_52_week_price,
                profile.beta,
                profile.earnings_per_share
            );
        }
    })
    .await
}
