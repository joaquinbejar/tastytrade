//! Streams historical and live OHLC bars.
//!
//! Candles are the only route to a price series anywhere in this crate: there
//! is no REST endpoint for one. They are also the only subscription that needs
//! more than a symbol — a period and a start time — because the symbol the
//! venue is subscribed with carries the period: `AAPL{=5m}`.
//!
//! This subscribes to **two different periods for the same underlying**, which
//! is what proves the routing keeps them apart: five-minute bars and hourly
//! bars arrive under different symbols and never into each other's
//! subscription.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_candles
//! ```
//!
//! Sizing is worth knowing before choosing a start time. One-minute bars over
//! a day is roughly 1440 events; five-minute bars over a week about 2016;
//! thirty-minute bars over a month about 1440. `from_time` is required for
//! exactly that reason — without one the subscription replays an unbounded
//! history.

use chrono::{Duration as ChronoDuration, Utc};
use quote_streaming::{DEADLINE, connect, read_bounded, symbols};
use tastytrade::prelude::*;
use tracing::info;

/// Enough bars to see both periods arriving without waiting on a live market:
/// the history alone fills this.
const MAX_EVENTS: usize = 40;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let Some(tasty) = connect().await? else {
        return Ok(());
    };

    let watched = symbols("AAPL");
    let five_minutes = CandlePeriod::minutes(5)?;
    let hourly = CandlePeriod::hours(1)?;

    // Two days back. The history arrives immediately, so this is also what
    // makes the example produce output outside market hours.
    let from_time = Utc::now() - ChronoDuration::days(2);

    let mut streamer = tasty.create_quote_streamer().await?;

    // Two subscriptions, one per period. They could be one — the routing is by
    // streamer symbol, and the period is part of it — but two makes the
    // separation visible: whatever arrives on `five` is a five-minute bar.
    let mut five = streamer.create_sub([EventKind::Candle]).await?;
    let mut hour = streamer.create_sub([EventKind::Candle]).await?;

    five.add_candles(&watched, five_minutes, from_time).await?;
    hour.add_candles(&watched, hourly, from_time).await?;

    info!(
        "Subscribed from {} — {:?} and {:?}",
        from_time.to_rfc3339(),
        five.subscribed(),
        hour.subscribed()
    );

    // The symbols differ by their period suffix, which is the whole mechanism.
    println!("five-minute subscription watches {:?}", five.subscribed());
    println!("hourly subscription watches      {:?}", hour.subscribed());

    println!("\nfive-minute bars (up to {MAX_EVENTS}, or {DEADLINE:?} of silence):");
    read_bounded(&mut five, MAX_EVENTS, print_bar).await;

    println!("\nhourly bars:");
    read_bounded(&mut hour, MAX_EVENTS, print_bar).await;

    Ok(())
}

fn print_bar(symbol: &str, data: &EventData) {
    if let EventData::Candle(candle) = data {
        println!(
            "{symbol}: o {} h {} l {} c {} vol {} vwap {} (t {})",
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume,
            candle.vwap,
            candle.time
        );
    }
}
