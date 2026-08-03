//! Streams individual executions as they print.
//!
//! One event per trade, with the quote around it — busier than `Trade`, which
//! only carries the last price. Bounded at fifty events because a liquid
//! symbol produces them faster than a terminal can be read.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_time_and_sale
//! ```

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(EventKind::TimeAndSale, "SPY", 50, |symbol, data| {
        if let EventData::TimeAndSale(sale) = data {
            println!(
                "{symbol}: {} x {} on {} ({} side){}",
                sale.price,
                sale.size,
                sale.exchange_code,
                sale.aggressor_side,
                if sale.extended_trading_hours {
                    ", extended hours"
                } else {
                    ""
                }
            );
            println!("  bid {} ask {}", sale.bid_price, sale.ask_price);
        }
    })
    .await
}
