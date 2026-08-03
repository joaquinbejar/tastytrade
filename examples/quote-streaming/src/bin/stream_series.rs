//! Streams one option expiration's computed values for an underlying.
//!
//! One event per expiration: implied volatility, call and put volume, the
//! forward price, and the dividend and interest assumptions behind them.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_series
//! ```

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(EventKind::Series, "AAPL,SPY", 20, |symbol, data| {
        if let EventData::Series(series) = data {
            println!(
                "{symbol}: expiration {} volatility {} forward {}",
                series.expiration, series.volatility, series.forward_price
            );
            println!(
                "  calls {} puts {} ratio {}",
                series.call_volume, series.put_volume, series.put_call_ratio
            );
        }
    })
    .await
}
