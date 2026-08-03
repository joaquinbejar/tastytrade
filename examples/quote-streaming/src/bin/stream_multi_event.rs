//! Streams several event types on one channel.
//!
//! One subscription, many kinds. The channel is configured for exactly what
//! the subscription asked for — not a fixed list — so adding a kind here is
//! all it takes to receive it.
//!
//! Routing is keyed by streamer symbol **and** event type, which is why a
//! subscription that asks for four kinds gets those four and nothing else,
//! even when another subscription elsewhere is watching the same symbol for
//! something different.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_multi_event
//! ```

use quote_streaming::{DEADLINE, connect, read_bounded, symbols};
use tastytrade::prelude::*;
use tracing::info;

const MAX_EVENTS: usize = 40;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let Some(tasty) = connect().await? else {
        return Ok(());
    };

    let watched = symbols("AAPL,SPY");
    let kinds = [
        EventKind::Quote,
        EventKind::Trade,
        EventKind::Summary,
        EventKind::Profile,
    ];

    let mut streamer = tasty.create_quote_streamer().await?;
    let mut sub = streamer.create_sub(kinds);
    sub.add_symbols(&watched).await?;

    info!(
        "Watching {} symbol(s) for {} event type(s); up to {MAX_EVENTS} events or {DEADLINE:?}",
        watched.len(),
        kinds.len()
    );

    read_bounded(&mut sub, MAX_EVENTS, |symbol, data| {
        // Every event knows its own kind, so a caller routing on it does not
        // have to match all eleven variants.
        print!("[{}] {symbol}: ", data.kind());
        match data {
            EventData::Quote(quote) => {
                println!("{} / {}", quote.bid_price, quote.ask_price)
            }
            EventData::Trade(trade) => println!("{} x {}", trade.price, trade.size),
            EventData::Summary(summary) => println!(
                "o {} h {} l {} c {}",
                summary.day_open_price,
                summary.day_high_price,
                summary.day_low_price,
                summary.day_close_price
            ),
            EventData::Profile(profile) => {
                println!("{} ({})", profile.description, profile.trading_status)
            }
            other => println!("{other:?}"),
        }
    })
    .await;

    Ok(())
}
