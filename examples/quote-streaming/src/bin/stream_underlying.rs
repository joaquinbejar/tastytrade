//! Streams the option surface over an underlying.
//!
//! Implied volatility across the surface plus call and put volumes. Only
//! arrives for symbols that have listed options.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_underlying
//! ```

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(EventKind::Underlying, "AAPL,SPY", 10, |symbol, data| {
        if let EventData::Underlying(underlying) = data {
            println!(
                "{symbol}: volatility {} (front {}, back {})",
                underlying.volatility, underlying.front_volatility, underlying.back_volatility
            );
            println!(
                "  calls {} puts {} ratio {}",
                underlying.call_volume, underlying.put_volume, underlying.put_call_ratio
            );
        }
    })
    .await
}
