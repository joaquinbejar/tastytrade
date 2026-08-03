//! Shared scaffolding for the streaming examples.
//!
//! Nine binaries in `src/bin/` all need the same four things: credentials from
//! the environment, a streamer, a bounded read loop, and an exit that happens
//! whether or not the market is saying anything. This is that, so each example
//! is about the event type it demonstrates and nothing else.

use std::time::Duration;

use tastytrade::prelude::*;
use tokio::time::timeout;
use tracing::info;

/// How long to wait for the next event before giving up.
///
/// Every example has to end. Half of these event types are silent outside
/// market hours — a `Profile` arrives once and then not again, a `TradeETH`
/// only in the extended session — so an example that waited for one would hang
/// on a quiet afternoon and could never be checked.
pub const DEADLINE: Duration = Duration::from_secs(30);

/// Connects using the environment, or explains what is missing.
///
/// `Ok(None)` when there are no credentials: an example that cannot run is not
/// a failure, and returning an error here would make `cargo run` look broken
/// on a fresh checkout.
pub async fn connect() -> Result<Option<TastyTrade>, Box<dyn std::error::Error>> {
    let config = TastyTradeConfig::from_env();

    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(None);
    }

    let tasty = TastyTrade::connect(&config).await?;
    info!("Authenticated against {}", config.environment());
    Ok(Some(tasty))
}

/// The symbols an example watches.
///
/// `TASTYTRADE_EXAMPLE_SYMBOLS` overrides the default, comma separated, so an
/// example can be pointed at an option or a future without editing it.
pub fn symbols(default: &str) -> Vec<Symbol> {
    std::env::var("TASTYTRADE_EXAMPLE_SYMBOLS")
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
        .map(Symbol::from)
        .collect()
}

/// Subscribes to `kind` for `default_symbols` and prints `describe` for each
/// event, stopping after `max_events` or [`DEADLINE`] of silence.
pub async fn stream_one(
    kind: EventKind,
    default_symbols: &str,
    max_events: usize,
    describe: impl Fn(&str, &EventData),
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(tasty) = connect().await? else {
        return Ok(());
    };

    let symbols = symbols(default_symbols);
    let mut streamer = tasty.create_quote_streamer().await?;
    // The channel is configured for exactly this event type, because that is
    // what the subscription asked for.
    let mut sub = streamer.create_sub([kind]);
    sub.add_symbols(&symbols).await?;

    info!(
        "Subscribed to {kind} for {} symbol(s); up to {max_events} events or {DEADLINE:?}",
        symbols.len()
    );

    read_bounded(&mut sub, max_events, describe).await;
    Ok(())
}

/// The bounded read loop the examples share.
pub async fn read_bounded(
    sub: &mut QuoteSubscription,
    max_events: usize,
    describe: impl Fn(&str, &EventData),
) {
    for _ in 0..max_events {
        match timeout(DEADLINE, sub.get_event()).await {
            Ok(Ok(event)) => describe(&event.sym, &event.data),
            Ok(Err(_)) => {
                info!("The stream ended");
                return;
            }
            Err(_) => {
                info!("Nothing further within {DEADLINE:?}; stopping");
                return;
            }
        }
    }
}
