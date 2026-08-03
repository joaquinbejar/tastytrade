//! Streams theoretical option prices.
//!
//! A `TheoPrice` is about a single **option** contract, so the default symbol
//! here will produce nothing useful: point it at an option streamer symbol.
//!
//! ```shell
//! TASTYTRADE_EXAMPLE_SYMBOLS='.AAPL260918C200' \
//!   cargo run -p quote-streaming --bin stream_theo_price
//! ```
//!
//! Get the streamer symbol for a contract with
//! `TastyTrade::get_streamer_symbol`; it is not always the instrument symbol.

use quote_streaming::stream_one;
use tastytrade::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    stream_one(
        EventKind::TheoPrice,
        ".SPY260320C500",
        10,
        |symbol, data| {
            if let EventData::TheoPrice(theo) = data {
                println!(
                    "{symbol}: theoretical {} against underlying {}",
                    theo.price, theo.underlying_price
                );
                println!(
                    "  delta {} gamma {}, dividend {} interest {}",
                    theo.delta, theo.gamma, theo.dividend, theo.interest
                );
            }
        },
    )
    .await
}
