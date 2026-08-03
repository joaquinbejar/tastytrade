//! Streams extended-hours trade prints.
//!
//! Between 04:00–09:30 and 16:00–20:00 ET, `Trade` is silent and `TradeETH` is
//! the only print there is — so this is the only route to an extended-hours
//! last price anywhere in this crate. Outside those windows it will
//! legitimately print nothing, which is why the example says up front which
//! session it is reading.
//!
//! ```shell
//! cargo run -p quote-streaming --bin stream_trade_eth
//! ```

use chrono::{Timelike, Utc};
use quote_streaming::stream_one;
use tastytrade::prelude::*;
use tracing::info;

/// Which US equity session the current time falls in.
///
/// Approximate on purpose: it uses a fixed −4 hour offset rather than a
/// timezone database, so it is right during daylight saving and an hour out
/// otherwise, and it knows nothing about market holidays. It exists to tell
/// the operator why nothing is printing, not to gate anything.
fn session_now() -> &'static str {
    let minutes = {
        let now = Utc::now();
        let hour = (now.hour() as i64 - 4).rem_euclid(24);
        hour * 60 + now.minute() as i64
    };

    match minutes {
        m if (240..570).contains(&m) => "pre-market (04:00–09:30 ET): TradeETH only",
        m if (570..960).contains(&m) => {
            "regular session (09:30–16:00 ET): TradeETH is normally silent"
        }
        m if (960..1200).contains(&m) => "post-market (16:00–20:00 ET): TradeETH only",
        _ => "closed (20:00–04:00 ET): nothing is printing anywhere",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    info!("Approximate session right now — {}", session_now());

    stream_one(EventKind::TradeEth, "AAPL,SPY", 20, |symbol, data| {
        if let EventData::TradeEth(trade) = data {
            println!(
                "{symbol}: {} x {} on {} ({}), change {}",
                trade.price, trade.size, trade.exchange_code, trade.tick_direction, trade.change
            );
            println!(
                "  extended hours: {}, session volume {}, turnover {}",
                trade.extended_trading_hours, trade.day_volume, trade.day_turnover
            );
        }
    })
    .await
}
