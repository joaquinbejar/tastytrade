// For quote_streamer.rs
use crate::TastyTrade;
use crate::streaming::reconnect::{BackoffPolicy, ConnectionState};
use crate::types::dxfeed;
use crate::types::dxfeed::{CandlePeriod, EventKind};
use crate::{AsSymbol, TastyResult, TastyTradeError};
use chrono::{DateTime, Utc};
use dxlink::{DXLinkClient, EventType, FeedSubscription, MarketEvent};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, error, info, warn};

#[derive(DebugPretty, DisplaySimple, Serialize, PartialEq, Eq, Hash, Clone, Copy)]
/// Identifies one subscription within a streamer.
pub struct SubscriptionId(usize);

/// A cheap, clonable handle to the streamer's command loop and its channel.
///
/// This is everything a subscription needs from the streamer: a way to send
/// commands. It used to hold a whole cloned `QuoteStreamer` instead, and that
/// clone's `Drop` sent `Disconnect` on the shared command channel, so dropping
/// a subscription tore down the connection the real streamer was still using.
/// A handle owns no connection, so dropping one cannot end anybody's stream.
#[derive(Clone)]
struct StreamerHandle {
    commands: Option<mpsc::Sender<DXLinkCommand>>,
}

/// A set of symbols and event types, and the events they produce.
///
/// Holds a handle to the streamer rather than the streamer itself, so dropping
/// a subscription cannot end the connection.
pub struct QuoteSubscription {
    /// This subscription's identity within its streamer.
    pub id: SubscriptionId,
    streamer: StreamerHandle,
    /// The event types this subscription asked for.
    ///
    /// Was an `i32` bitmask of three `DXF_ET_*` constants. A typed set, so the
    /// eleven the feed models are all reachable and a caller cannot ask for a
    /// bit that means nothing.
    kinds: BTreeSet<EventKind>,
    event_receiver: flume::Receiver<dxfeed::Event>, // Keep for compatibility
    dxlink_receiver: mpsc::Receiver<MarketEvent>,   // New DXLink event receiver
    /// What this subscription is actually subscribed to.
    ///
    /// Shared with the copy the streamer keeps in its `subscription_map`, and
    /// that sharing is the fix rather than an optimisation: `create_sub`
    /// stores one clone and hands the caller another, so a `Vec` on each
    /// meant `add_symbols` updated the caller's copy while `close_sub` read
    /// the streamer's, which stayed empty forever. Unsubscribes were
    /// therefore derived from an empty list and never sent, leaving the
    /// subscription alive on the venue.
    ///
    /// A set, so asking twice subscribes once.
    targets: Arc<Mutex<BTreeSet<FeedTarget>>>,
    /// How many events this subscription has lost by not keeping up.
    ///
    /// Shared with the sink the forwarder delivers into, so it counts what
    /// actually happened to *this* consumer.
    lagged: Arc<AtomicU64>,
}

impl QuoteSubscription {
    /// Subscribes this subscription to `symbols`.
    ///
    /// Returns once the venue has accepted the subscription, not merely once
    /// the command was queued. Symbols already subscribed are skipped, so
    /// calling twice with the same symbol subscribes once.
    ///
    /// Subscribes each symbol under every event type this subscription asked
    /// for **except candles**: a candle needs a period and a history start,
    /// and a bare symbol has neither. Use [`QuoteSubscription::add_candles`].
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Precondition`] when the subscription asked for
    /// nothing but candles, because there is then nothing this call can
    /// subscribe. Otherwise fails when the streamer has no open channel, when
    /// it is closed, or when the venue refuses. On any of those the symbols
    /// are not recorded, so a later close does not try to unsubscribe
    /// something that was never subscribed.
    pub async fn add_symbols<S: AsSymbol>(&self, symbols: &[S]) -> TastyResult<()> {
        let kinds: Vec<EventKind> = self
            .kinds
            .iter()
            .copied()
            .filter(|kind| !kind.needs_a_period())
            .collect();

        if kinds.is_empty() {
            return Err(TastyTradeError::Precondition(
                "this subscription asked for candles only, and a candle needs a period and a \
                 start time; use add_candles"
                    .to_string(),
            ));
        }

        let requested: Vec<FeedTarget> = symbols
            .iter()
            .flat_map(|symbol| {
                let symbol = symbol.as_symbol();
                kinds.iter().map(move |kind| FeedTarget {
                    kind: *kind,
                    symbol: symbol.0.clone(),
                    from_time: None,
                })
            })
            .collect();

        self.subscribe_targets(requested).await
    }

    /// Subscribes this subscription to candles for `symbols`.
    ///
    /// A candle subscription is addressed by a symbol that carries its own
    /// period — `AAPL{=5m}` — so two periods of one underlying are two
    /// different streamer symbols and never deliver into each other. The
    /// events come back under that same symbol, which is what
    /// [`dxfeed::Event::sym`] holds.
    ///
    /// `from_time` is required, not optional. A candle subscription without
    /// one replays an unbounded history: the documented sizing is about 1440
    /// events for a day of one-minute bars, and there is no upper bound at
    /// all on "everything you have".
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Precondition`] when the subscription did not ask for
    /// [`EventKind::Candle`] — a channel is only configured for the types its
    /// subscriptions requested, so this would subscribe to something that
    /// cannot arrive. Otherwise as [`QuoteSubscription::add_symbols`].
    pub async fn add_candles<S: AsSymbol>(
        &self,
        symbols: &[S],
        period: CandlePeriod,
        from_time: DateTime<Utc>,
    ) -> TastyResult<()> {
        if !self.kinds.contains(&EventKind::Candle) {
            return Err(TastyTradeError::Precondition(
                "this subscription did not ask for candles, so the channel is not configured to \
                 deliver them; create one with EventKind::Candle"
                    .to_string(),
            ));
        }

        // Milliseconds. dxlink documents `FeedSubscription::from_time` as a
        // Unix timestamp in milliseconds and it is the code that serialises
        // it, so that is what this follows. Taking a `DateTime` rather than an
        // integer keeps the choice in one place instead of at every call site.
        let from_time = from_time.timestamp_millis();

        let requested: Vec<FeedTarget> = symbols
            .iter()
            .map(|symbol| FeedTarget {
                kind: EventKind::Candle,
                symbol: period.streamer_symbol(&symbol.as_symbol().0),
                from_time: Some(from_time),
            })
            .collect();

        self.subscribe_targets(requested).await
    }

    /// How many events this subscription has lost by not keeping up.
    ///
    /// Zero means the stream is complete as far as this subscription is
    /// concerned — which is the question a caller assembling a price series
    /// actually needs answered, and had no way to ask. A non-zero count is not
    /// recoverable by reading faster: those events are gone.
    ///
    /// Candles are the exception, and only across a reconnect: a bar that was
    /// dropped stops the resume point advancing, so the next connection asks
    /// for it again. Within one connection a dropped bar stays dropped.
    pub fn lagged(&self) -> u64 {
        self.lagged.load(Ordering::Relaxed)
    }

    /// Every streamer symbol this subscription is subscribed to, with its
    /// event type.
    ///
    /// Candle symbols carry their period, which is how a caller tells
    /// `AAPL{=5m}` from `AAPL{=1h}`.
    pub fn subscribed(&self) -> Vec<(String, EventKind)> {
        targets_of(&self.targets)
            .iter()
            .map(|target| (target.symbol.clone(), target.kind))
            .collect()
    }

    /// Reserves `requested`, asks the venue, and gives the reservation back if
    /// it is refused.
    async fn subscribe_targets(&self, requested: Vec<FeedTarget>) -> TastyResult<()> {
        // Checked and reserved in one lock section. Filtering against the set
        // and inserting afterwards let two concurrent callers both see a
        // target as absent and both subscribe to it. Reserving here means the
        // second caller sees the first one's claim; a failure below removes
        // the reservation again.
        let targets: Vec<FeedTarget> = {
            let mut known = targets_of(&self.targets);
            requested
                .into_iter()
                .filter(|target| known.insert(target.clone()))
                .collect()
        };

        let subscriptions = feed_subscriptions(&targets);

        if subscriptions.is_empty() {
            return Ok(());
        }

        // Awaited rather than spawned. A detached task meant this returned
        // success before the command was even accepted, so a caller could not
        // tell a subscription that worked from one that never left, and it
        // panicked outright when called without a Tokio runtime.
        let sub_id = self.id.0 as u32;
        let Some(tx) = &self.streamer.commands else {
            let mut known = targets_of(&self.targets);
            for target in &targets {
                known.remove(target);
            }
            return Err(TastyTradeError::Streaming(
                "the quote streamer has no command channel; reconnect before subscribing"
                    .to_string(),
            ));
        };

        let (ack, answered) = oneshot::channel();
        let queued = tx
            .send(DXLinkCommand::Subscribe(
                subscriptions,
                targets.iter().map(|target| target.kind).collect(),
                sub_id,
                Some(ack),
            ))
            .await
            .map_err(|_| {
                TastyTradeError::Streaming(
                    "the quote streamer is closed; reconnect before subscribing".to_string(),
                )
            });

        // Reaching the command queue is not the venue accepting the
        // subscription: the loop can still be refused by DXLink. Wait for the
        // real answer, and give back the reservation if it is a refusal, so a
        // target that is not subscribed is never later unsubscribed as though
        // it were.
        let outcome = match queued {
            Ok(()) => answered.await.unwrap_or_else(|_| {
                Err(TastyTradeError::Streaming(
                    "the quote streamer closed before the subscription was confirmed".to_string(),
                ))
            }),
            Err(e) => Err(e),
        };

        if outcome.is_err() {
            let mut known = targets_of(&self.targets);
            for target in &targets {
                known.remove(target);
            }
        }

        outcome
    }

    /// Receive one event from feed. Yields if there are no events.
    /// Compatible with previous interface
    pub async fn get_event(&mut self) -> Result<dxfeed::Event, flume::RecvError> {
        // A loop rather than one match: dxlink decodes event types this crate
        // has no `EventData` for, and one of those arriving must not look to
        // the caller like the stream ended.
        loop {
            let Some(market_event) = self.dxlink_receiver.recv().await else {
                // Every sender is gone, which is what happens when the
                // streamer is dropped. The flume receiver behind this is
                // already disconnected, so this reports the end rather than
                // waiting for something that cannot arrive.
                return self.event_receiver.recv_async().await;
            };

            let converted = convert_event(market_event);

            if let Some(event) = converted {
                return Ok(event);
            }
        }
    }
}

/// Converts one dxlink event into this crate's own event type.
///
/// `None` only for an event whose symbol this crate cannot read, which cannot
/// happen for a variant that is modelled — every one of the eleven carries an
/// `eventSymbol`. It exists so the caller of this function does not have to
/// care, and so a future variant does not silently become a `Quote`.
///
/// `f64` throughout, and only here: `types::dxfeed` holds the native feed
/// types, where the representation is the feed's to choose. Nothing on the
/// REST path is allowed to widen that.
fn convert_event(event: MarketEvent) -> Option<dxfeed::Event> {
    let data = match event {
        MarketEvent::Quote(quote) => {
            return Some(dxfeed::Event {
                sym: quote.event_symbol,
                data: dxfeed::EventData::Quote(dxfeed::DxfQuoteT {
                    time: 0,
                    sequence: 0,
                    time_nanos: 0,
                    bid_time: 0,
                    bid_exchange_code: 0,
                    bid_price: quote.bid_price,
                    ask_price: quote.ask_price,
                    bid_size: quote.bid_size as i64,
                    ask_time: 0,
                    ask_size: quote.ask_size as i64,
                    ask_exchange_code: 0,
                    scope: 0,
                }),
            });
        }
        MarketEvent::Trade(trade) => {
            return Some(dxfeed::Event {
                sym: trade.event_symbol,
                data: dxfeed::EventData::Trade(dxfeed::DxfTradeT {
                    time: 0,
                    sequence: 0,
                    time_nanos: 0,
                    exchange_code: 0,
                    price: trade.price,
                    size: trade.size as i64,
                    tick: 0,
                    change: 0.0,
                    day_id: 0,
                    day_volume: trade.day_volume,
                    day_turnover: 0.0,
                    raw_flags: 0,
                    direction: 0,
                    is_eth: 0,
                    scope: 0,
                }),
            });
        }
        MarketEvent::Greeks(greeks) => {
            return Some(dxfeed::Event {
                sym: greeks.event_symbol,
                // DXLink's GreeksEvent carries no price or time, so those stay 0.
                data: dxfeed::EventData::Greeks(dxfeed::DxfGreeksT {
                    event_flags: 0,
                    index: 0,
                    time: 0,
                    price: 0.0,
                    volatility: greeks.volatility,
                    delta: greeks.delta,
                    gamma: greeks.gamma,
                    theta: greeks.theta,
                    vega: greeks.vega,
                    rho: greeks.rho,
                }),
            });
        }
        MarketEvent::TradeETH(trade) => (
            trade.event_symbol.clone(),
            dxfeed::EventData::TradeEth(Box::new(dxfeed::DxfTradeEthT {
                event_time: trade.event_time,
                time: trade.time,
                time_nano_part: trade.time_nano_part,
                sequence: trade.sequence,
                exchange_code: trade.exchange_code,
                price: trade.price,
                change: trade.change,
                size: trade.size,
                day_id: trade.day_id,
                day_volume: trade.day_volume,
                day_turnover: trade.day_turnover,
                tick_direction: trade.tick_direction,
                extended_trading_hours: trade.extended_trading_hours,
            })),
        ),
        MarketEvent::Candle(candle) => (
            candle.event_symbol.clone(),
            dxfeed::EventData::Candle(Box::new(dxfeed::DxfCandleT {
                event_time: candle.event_time,
                event_flags: candle.event_flags,
                index: candle.index,
                time: candle.time,
                sequence: candle.sequence,
                count: candle.count,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
                vwap: candle.vwap,
                bid_volume: candle.bid_volume,
                ask_volume: candle.ask_volume,
                imp_volatility: candle.imp_volatility,
                open_interest: candle.open_interest,
            })),
        ),
        MarketEvent::Summary(summary) => (
            summary.event_symbol.clone(),
            dxfeed::EventData::Summary(Box::new(dxfeed::DxfSummaryT {
                event_time: summary.event_time,
                day_id: summary.day_id,
                day_open_price: summary.day_open_price,
                day_high_price: summary.day_high_price,
                day_low_price: summary.day_low_price,
                day_close_price: summary.day_close_price,
                day_close_price_type: summary.day_close_price_type,
                prev_day_id: summary.prev_day_id,
                prev_day_close_price: summary.prev_day_close_price,
                prev_day_close_price_type: summary.prev_day_close_price_type,
                prev_day_volume: summary.prev_day_volume,
                open_interest: summary.open_interest,
            })),
        ),
        MarketEvent::TimeAndSale(sale) => (
            sale.event_symbol.clone(),
            dxfeed::EventData::TimeAndSale(Box::new(dxfeed::DxfTimeAndSaleT {
                event_time: sale.event_time,
                event_flags: sale.event_flags,
                index: sale.index,
                time: sale.time,
                time_nano_part: sale.time_nano_part,
                sequence: sale.sequence,
                exchange_code: sale.exchange_code,
                price: sale.price,
                size: sale.size,
                bid_price: sale.bid_price,
                ask_price: sale.ask_price,
                exchange_sale_conditions: sale.exchange_sale_conditions,
                trade_through_exempt: sale.trade_through_exempt,
                aggressor_side: sale.aggressor_side,
                spread_leg: sale.spread_leg,
                extended_trading_hours: sale.extended_trading_hours,
                valid_tick: sale.valid_tick,
                sale_type: sale.sale_type,
                buyer: sale.buyer,
                seller: sale.seller,
            })),
        ),
        MarketEvent::Profile(profile) => (
            profile.event_symbol.clone(),
            dxfeed::EventData::Profile(Box::new(dxfeed::DxfProfileT {
                event_time: profile.event_time,
                description: profile.description,
                short_sale_restriction: profile.short_sale_restriction,
                trading_status: profile.trading_status,
                status_reason: profile.status_reason,
                halt_start_time: profile.halt_start_time,
                halt_end_time: profile.halt_end_time,
                high_limit_price: profile.high_limit_price,
                low_limit_price: profile.low_limit_price,
                high_52_week_price: profile.high_52_week_price,
                low_52_week_price: profile.low_52_week_price,
                beta: profile.beta,
                earnings_per_share: profile.earnings_per_share,
                dividend_frequency: profile.dividend_frequency,
                ex_dividend_amount: profile.ex_dividend_amount,
                ex_dividend_day_id: profile.ex_dividend_day_id,
                shares: profile.shares,
                free_float: profile.free_float,
            })),
        ),
        MarketEvent::Underlying(underlying) => (
            underlying.event_symbol.clone(),
            dxfeed::EventData::Underlying(Box::new(dxfeed::DxfUnderlyingT {
                event_time: underlying.event_time,
                event_flags: underlying.event_flags,
                index: underlying.index,
                time: underlying.time,
                sequence: underlying.sequence,
                volatility: underlying.volatility,
                front_volatility: underlying.front_volatility,
                back_volatility: underlying.back_volatility,
                call_volume: underlying.call_volume,
                put_volume: underlying.put_volume,
                put_call_ratio: underlying.put_call_ratio,
            })),
        ),
        MarketEvent::TheoPrice(theo) => (
            theo.event_symbol.clone(),
            dxfeed::EventData::TheoPrice(Box::new(dxfeed::DxfTheoPriceT {
                event_time: theo.event_time,
                event_flags: theo.event_flags,
                index: theo.index,
                time: theo.time,
                sequence: theo.sequence,
                price: theo.price,
                underlying_price: theo.underlying_price,
                delta: theo.delta,
                gamma: theo.gamma,
                dividend: theo.dividend,
                interest: theo.interest,
            })),
        ),
        MarketEvent::Series(series) => (
            series.event_symbol.clone(),
            dxfeed::EventData::Series(Box::new(dxfeed::DxfSeriesT {
                event_time: series.event_time,
                event_flags: series.event_flags,
                index: series.index,
                time: series.time,
                sequence: series.sequence,
                expiration: series.expiration,
                volatility: series.volatility,
                call_volume: series.call_volume,
                put_volume: series.put_volume,
                put_call_ratio: series.put_call_ratio,
                forward_price: series.forward_price,
                dividend: series.dividend,
                interest: series.interest,
            })),
        ),
    };

    Some(dxfeed::Event {
        sym: data.0,
        data: data.1,
    })
}

// Commands for DXLink client to execute.
// Subscribe/Unsubscribe carry the subscription id so events can be routed
// back to the subscription that requested each symbol.
/// Replies to whoever is waiting on a command, if anyone still is.
///
/// A caller that stopped waiting is not an error: dropping the receiver is how
/// a fire-and-forget caller opts out.
fn answer(ack: Option<oneshot::Sender<TastyResult<()>>>, outcome: TastyResult<()>) {
    if let Some(ack) = ack {
        let _ = ack.send(outcome);
    }
}

/// How many events a subscription may fall behind by before losing some.
///
/// A candle history is the reason this is not the old hundred: a day of
/// one-minute bars is about 1440 events and they arrive at once, so a hundred
/// meant a caller reading a series lost most of it before it ever looked.
/// Large enough to absorb a documented history, still bounded — an unbounded
/// channel turns a slow consumer into unbounded memory.
///
/// A default rather than a rule: a caller who knows their own history size
/// chooses with [`QuoteStreamer::create_sub_with_capacity`].
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 4096;

/// One consumer of a subscription's events, and what it has lost.
///
/// The counter travels with the sender so the forwarder can charge a drop to
/// the consumer it actually happened to, rather than to a symbol or a
/// subscription that has several.
#[derive(Clone)]
struct Subscriber {
    events: mpsc::Sender<MarketEvent>,
    lagged: Arc<AtomicU64>,
}

/// Where a candle subscription should resume, per subscription and symbol.
///
/// `through` is the last bar **delivered contiguously** — not the highest one
/// seen. The difference is the whole point: a maximum steps over a gap, and a
/// gap in a price series is invisible to everything downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CandleResume {
    /// The last bar handed to the consumer with nothing missing before it.
    through: i64,
    /// Whether a bar has been dropped since. While this is set, `through`
    /// stops advancing, so the reconnect asks for everything from the last
    /// known-good bar and the gap is refilled.
    gap: bool,
}

/// One thing a subscription is subscribed to.
///
/// A symbol alone is not enough to describe a subscription any more. Candles
/// are addressed by a symbol that carries their period — `AAPL{=5m}` — so two
/// periods of one underlying are two different streamer symbols, and each
/// needs its own history start. Routing, unsubscribing and the reconnect
/// replay all work from these.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FeedTarget {
    /// The event type this target is for.
    ///
    /// Part of the identity: a subscription that asked for Quote on AAPL must
    /// not receive the Trade prints another subscription asked for.
    kind: EventKind,
    /// The streamer symbol exactly as the venue is told it, period suffix
    /// included. It is also the `eventSymbol` the events come back under.
    symbol: String,
    /// Where a candle history starts, in epoch milliseconds.
    ///
    /// `None` for everything else. A candle subscription without one replays
    /// an unbounded history, which is why `add_candles` requires it.
    from_time: Option<i64>,
}

/// The DXLink subscription requests for `targets`.
///
/// Shared by subscribing, unsubscribing and the reconnect replay, so a target
/// is restored under exactly the event type, symbol and history start it was
/// subscribed with.
fn feed_subscriptions(targets: &[FeedTarget]) -> Vec<FeedSubscription> {
    targets
        .iter()
        .map(|target| FeedSubscription {
            event_type: target.kind.wire_name().to_string(),
            symbol: target.symbol.clone(),
            from_time: target.from_time,
            source: None,
        })
        .collect()
}

/// Recovers a poisoned lock rather than panicking.
///
/// The value behind it is a set of subscription targets. A thread panicking
/// while holding the lock cannot leave that set in a state the next reader
/// cannot understand, so poisoning here carries no information worth aborting
/// a caller's process over.
fn targets_of(
    set: &Mutex<BTreeSet<FeedTarget>>,
) -> std::sync::MutexGuard<'_, BTreeSet<FeedTarget>> {
    set.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

enum DXLinkCommand {
    // No channel id: the caller's copy is a snapshot from connect time, and a
    // reconnect opens a new channel. The supervisor addresses the live one.
    // The kinds travel with the request: the channel has to be configured for
    // an event type before the venue will accept a subscription to it, and
    // only this loop knows what the live channel is already configured for.
    Subscribe(
        Vec<FeedSubscription>,
        BTreeSet<EventKind>,
        u32,
        Option<oneshot::Sender<TastyResult<()>>>,
    ),
    Unsubscribe(
        Vec<FeedSubscription>,
        u32,
        Option<oneshot::Sender<TastyResult<()>>>,
    ),
    AddEventSender(u32, Subscriber),
    RemoveEventSender(u32),
}

// Live routing registry shared between the command loop and the event
// forwarding task, so senders registered at any time are always visible.
#[derive(Default)]
struct EventRouting {
    senders: HashMap<u32, Vec<Subscriber>>,
    /// Which subscriptions want which `(streamer symbol, event type)`.
    ///
    /// Keyed by both halves, not by the symbol alone. The symbol carries a
    /// candle's period — `AAPL{=5m}` — so two periods of one underlying are
    /// already distinct; the event type is what stops a subscription that
    /// asked for Quote on AAPL from receiving the Trade prints another
    /// subscription asked for.
    routes: HashMap<(String, EventKind), HashSet<u32>>,
}

/// Where each subscription's candle series should resume.
///
/// Keyed by **subscription and symbol**, not by symbol alone. Two
/// subscriptions can watch the same bars and fall behind by different amounts,
/// and a replay is per subscription — so a shared counter would let a
/// subscription that kept up decide where a subscription that did not resumes
/// from. The symbol carries its period, which keeps `AAPL{=5m}` and
/// `AAPL{=1h}` apart within one subscription.
type CandleProgress = Arc<Mutex<HashMap<(u32, String), CandleResume>>>;

/// Owns the DXLink connection and the subscriptions on it.
///
/// Deliberately not `Clone`: exactly one value owns the connection, and
/// dropping it disconnects. Subscriptions get a handle instead.
pub struct QuoteStreamer {
    /// Signals the supervisor to disconnect.
    ///
    /// Held only by the owner, never by a handle, so it is dropped exactly when
    /// the streamer is. A oneshot cannot be refused for lack of room the way a
    /// `try_send` into the bounded command queue could, and the supervisor
    /// selects on it during a backoff so a dropped streamer does not leave a
    /// task waiting out thirty seconds for nobody.
    shutdown: Option<oneshot::Sender<()>>,
    next_sub_id: usize,
    subscription_map: HashMap<SubscriptionId, QuoteSubscription>,
    dxlink_command_tx: Option<mpsc::Sender<DXLinkCommand>>,
    /// What a reconnect has to restore, shared with the supervisor.
    registry: Registry,
    state: Arc<RwLock<ConnectionState>>,
}

/// What each subscription is subscribed to, as the supervisor needs it.
///
/// The symbol set is the same `Arc` the subscription and the streamer share,
/// so a replay sends exactly what the venue confirmed — never a symbol whose
/// subscribe was refused, and never one that was already unsubscribed.
#[derive(Clone)]
struct SubscriptionRecord {
    kinds: BTreeSet<EventKind>,
    targets: Arc<Mutex<BTreeSet<FeedTarget>>>,
}

/// Subscription id to what it is subscribed to.
type Registry = Arc<Mutex<HashMap<u32, SubscriptionRecord>>>;

impl QuoteStreamer {
    /// Opens a DXLink connection and its market-data channel.
    ///
    /// Reconnects under [`BackoffPolicy::default`]; see
    /// [`QuoteStreamer::connect_with_policy`] to choose your own.
    ///
    /// # Errors
    ///
    /// Fails when the streamer token cannot be obtained, the connection
    /// cannot be established, or the channel cannot be configured.
    pub async fn connect(tasty: &TastyTrade) -> TastyResult<Self> {
        Self::connect_with_policy(tasty, BackoffPolicy::default()).await
    }

    /// Opens a DXLink connection with an explicit reconnection policy.
    ///
    /// The first connection is established before returning, so a caller who
    /// cannot reach the venue at all learns immediately rather than through a
    /// stream that never produces anything. After that a supervisor owns the
    /// connection: when it is lost, the supervisor waits out the backoff,
    /// fetches a fresh streamer token, reconnects, and resubscribes every
    /// symbol the subscriptions were subscribed to. Subscriptions keep working
    /// across that — they hold a handle to the command loop, not to the
    /// connection.
    ///
    /// # Errors
    ///
    /// Fails when the streamer token cannot be obtained, the connection
    /// cannot be established, or the channel cannot be configured.
    pub async fn connect_with_policy(
        tasty: &TastyTrade,
        policy: BackoffPolicy,
    ) -> TastyResult<Self> {
        // Prove the venue is reachable before handing back a streamer.
        let connection = connect_dxlink(tasty).await?;

        let (command_tx, command_rx) = mpsc::channel::<DXLinkCommand>(100);

        // Shutdown travels on its own channel. Routing it through the bounded
        // command queue meant a full queue could discard it, and the loop
        // cannot simply exit when the command sender dies either, because
        // every subscription handle holds a clone of that sender.
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Owned out here, so it survives a reconnect. A subscription's route
        // is registered once; rebuilding the map per connection would drop the
        // first events of every reconnect, and re-registering is not something
        // a caller can be asked to do.
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let registry: Registry = Arc::new(Mutex::new(HashMap::new()));
        let state = Arc::new(RwLock::new(ConnectionState::Connected));
        // Outside the connection loop for the same reason `routing` is: a
        // reconnect must resume a candle series where the consumer was left,
        // and a per-connection map would forget that every time.
        let progress: CandleProgress = Arc::new(Mutex::new(HashMap::new()));

        tokio::spawn(supervise(
            tasty.clone(),
            policy,
            connection,
            command_rx,
            shutdown_rx,
            routing,
            registry.clone(),
            state.clone(),
            progress,
        ));

        Ok(Self {
            shutdown: Some(shutdown_tx),
            next_sub_id: 0,
            subscription_map: HashMap::new(),
            dxlink_command_tx: Some(command_tx),
            registry,
            state,
        })
    }

    /// Where the connection is in its lifecycle.
    ///
    /// Carries no token, credential or account identifier, so it is safe to
    /// log or show. A reconnect that happens silently is indistinguishable
    /// from one that is not happening, which is what this exists to answer.
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// A handle a subscription can hold without owning the connection.
    fn handle(&self) -> StreamerHandle {
        StreamerHandle {
            commands: self.dxlink_command_tx.clone(),
        }
    }

    /// Creates a subscription for the given event types.
    ///
    /// Was `create_sub(flags: i32)`, a raw dxfeed bitmask a caller had to know
    /// the constants for, covering three of the eleven types the feed models.
    /// **Breaking**, and deliberately so: the replacement is a typed set, and
    /// there is no bit pattern that silently means "nothing".
    ///
    /// Including [`EventKind::Candle`] is what allows
    /// [`QuoteSubscription::add_candles`]; candles are not subscribed by
    /// [`QuoteSubscription::add_symbols`], because a bare symbol has neither a
    /// period nor a history start.
    ///
    /// **`async`, and that is the fix rather than an inconvenience.** The
    /// returned subscription's event route is registered with the command loop
    /// *before* this returns. It used to be registered from a detached
    /// `tokio::spawn`, so a caller that subscribed immediately could have the
    /// subscribe reach the loop first — and a candle subscription's history
    /// arrives at once, so the first bars were routed to a subscription the
    /// loop did not know about yet and dropped.
    ///
    /// # Errors
    ///
    /// Fails when the streamer is closed, which is the only way registration
    /// can not happen. Returning a subscription that can never receive
    /// anything would be the same silent failure in a different place.
    pub async fn create_sub(
        &mut self,
        kinds: impl IntoIterator<Item = EventKind>,
    ) -> TastyResult<Box<QuoteSubscription>> {
        self.create_sub_with_capacity(kinds, DEFAULT_EVENT_CHANNEL_CAPACITY)
            .await
    }

    /// The same, with a chosen buffer size.
    ///
    /// The default absorbs a documented candle history — about 1440 events for
    /// a day of one-minute bars, arriving together — and a caller who knows
    /// their own history is larger, or who is reading a firehose of
    /// `TimeAndSale` slowly, should say so rather than discovering the number
    /// through [`QuoteSubscription::lagged`].
    ///
    /// Bounded either way. An unbounded channel does not remove the problem,
    /// it converts a slow consumer into unbounded memory.
    ///
    /// # Errors
    ///
    /// As [`QuoteStreamer::create_sub`], plus
    /// [`TastyTradeError::Precondition`] for a capacity of zero, which is a
    /// subscription that can never deliver anything.
    pub async fn create_sub_with_capacity(
        &mut self,
        kinds: impl IntoIterator<Item = EventKind>,
        capacity: usize,
    ) -> TastyResult<Box<QuoteSubscription>> {
        if capacity == 0 {
            return Err(TastyTradeError::Precondition(
                "a subscription with no buffer cannot deliver anything; it would drop every \
                 event and report itself as lagging"
                    .to_string(),
            ));
        }

        let kinds: BTreeSet<EventKind> = kinds.into_iter().collect();
        let id = SubscriptionId(self.next_sub_id);
        self.next_sub_id += 1;
        let sub_id = id.0 as u32;

        let (caller_tx, caller_rx) = mpsc::channel(capacity);
        let lagged = Arc::new(AtomicU64::new(0));
        let (_event_sender, event_receiver) = flume::unbounded();

        let Some(commands) = &self.dxlink_command_tx else {
            return Err(TastyTradeError::Streaming(
                "the quote streamer has no command channel; reconnect before subscribing"
                    .to_string(),
            ));
        };

        // **One** registered consumer, the caller's. The streamer used to
        // register a second for the copy it keeps, and nothing could ever read
        // that one: `get_sub` hands out a `&QuoteSubscription` and `get_event`
        // needs `&mut`. So it filled to capacity and then charged a drop for
        // every event afterwards — manufacturing exactly the lag this change
        // exists to measure.
        commands
            .send(DXLinkCommand::AddEventSender(
                sub_id,
                Subscriber {
                    events: caller_tx,
                    lagged: lagged.clone(),
                },
            ))
            .await
            .map_err(|_| {
                TastyTradeError::Streaming(
                    "the quote streamer is closed; it cannot route events to a new subscription"
                        .to_string(),
                )
            })?;

        // Create subscription
        let targets = Arc::new(Mutex::new(BTreeSet::new()));

        // The supervisor replays from this. Registering the shared set rather
        // than a copy is what makes the replay send exactly what the venue
        // confirmed, including targets added long after this call.
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                sub_id,
                SubscriptionRecord {
                    kinds: kinds.clone(),
                    targets: targets.clone(),
                },
            );

        // The streamer's own copy exists for `close_sub` and `get_sub`, which
        // read its targets and its identity. Its event channel is closed from
        // the start rather than being fed into a void: a receiver nobody can
        // reach should say so if anybody ever asks it.
        let (_closed, closed_rx) = mpsc::channel(1);
        self.subscription_map.insert(
            id,
            QuoteSubscription {
                id,
                streamer: self.handle(),
                kinds: kinds.clone(),
                event_receiver: event_receiver.clone(),
                dxlink_receiver: closed_rx,
                targets: targets.clone(),
                lagged: Arc::new(AtomicU64::new(0)),
            },
        );

        Ok(Box::new(QuoteSubscription {
            id,
            streamer: self.handle(),
            kinds,
            event_receiver,
            dxlink_receiver: caller_rx,
            targets,
            lagged,
        }))
    }

    /// Retrieve a subscription by id.
    pub fn get_sub(&self, id: SubscriptionId) -> Option<&QuoteSubscription> {
        self.subscription_map.get(&id)
    }

    /// Unsubscribes a subscription's symbols and removes it.
    ///
    /// # Errors
    ///
    /// Fails when the venue refuses the unsubscribe or the streamer is
    /// closed. On failure the subscription's symbols stay recorded, because
    /// that record is what a retry needs.
    pub async fn close_sub(&mut self, id: SubscriptionId) -> TastyResult<()> {
        // Get symbols from subscription to close. This is the shared set, so
        // it holds what add_symbols actually subscribed rather than the empty
        // vector this used to read.
        if let Some(subscription) = self.subscription_map.get(&id) {
            let targets: Vec<FeedTarget> =
                targets_of(&subscription.targets).iter().cloned().collect();

            let unsubscribe_requests = feed_subscriptions(&targets);

            // Awaited, and the local state is only discarded once the venue
            // has confirmed. Clearing it on a queued-but-unconfirmed command
            // threw away the one record of what still needs unsubscribing.
            if let Some(tx) = &self.dxlink_command_tx {
                let sub_id = id.0 as u32;

                let closed = |_| {
                    TastyTradeError::Streaming(
                        "the quote streamer is closed; the subscription is gone with it"
                            .to_string(),
                    )
                };

                if !unsubscribe_requests.is_empty() {
                    let (ack, answered) = oneshot::channel();
                    tx.send(DXLinkCommand::Unsubscribe(
                        unsubscribe_requests,
                        sub_id,
                        Some(ack),
                    ))
                    .await
                    .map_err(closed)?;

                    answered.await.unwrap_or_else(|_| {
                        Err(TastyTradeError::Streaming(
                            "the quote streamer closed before the unsubscribe was confirmed"
                                .to_string(),
                        ))
                    })?;
                }

                // Only now: the venue has stopped sending, so there is nothing
                // left to route.
                tx.send(DXLinkCommand::RemoveEventSender(sub_id))
                    .await
                    .map_err(closed)?;
            }

            // Confirmed unsubscribed, so the shared set must stop claiming
            // otherwise. Reached only on success: an early return above leaves
            // the symbols recorded, which is what a retry needs.
            if let Some(subscription) = self.subscription_map.get(&id) {
                targets_of(&subscription.targets).clear();
            }
        }

        // Remove subscription from map, and from what a reconnect restores:
        // a closed subscription must not come back on the next connection.
        self.subscription_map.remove(&id);
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&(id.0 as u32));

        Ok(())
    }
}

/// One live DXLink connection and everything the supervisor needs from it.
struct LiveConnection {
    client: DXLinkClient,
    channel_id: u32,
    /// The receiver `DXLinkClient::connect` hands back.
    ///
    /// Two things at once. It is the *only* path market events take out of
    /// dxlink — the client's event sender is created inside `connect`, so a
    /// later `event_stream()` call is refused with "Event stream already
    /// created", and dropping this receiver disconnects the feed from its
    /// consumer permanently, which is what used to happen here. And it closes
    /// when the session ends, which is how a drop becomes visible without this
    /// client writing anything.
    events: mpsc::Receiver<MarketEvent>,
}

/// Opens a connection, its feed channel, and configures the event types.
async fn connect_dxlink(tasty: &TastyTrade) -> TastyResult<LiveConnection> {
    let tokens = tasty.quote_streamer_tokens().await?;
    // The token itself is never logged, here or anywhere: the streamer-token
    // response is a credential.
    debug!(
        "Obtained DXLink streamer token ({} bytes)",
        tokens.token.len()
    );

    // dxlink can reconnect on its own (`with_reconnect`), and it is
    // deliberately not installed: this crate already reconnects under
    // `BackoffPolicy`, replaying the symbols each subscription confirmed, and
    // two policies over one socket would count a single drop twice, under two
    // budgets, with two sets of attempt numbers reaching the caller through
    // `state()`. What is used from dxlink instead is the fact it reports the
    // session ending — the event stream closes — which is the part this crate
    // cannot observe for itself.
    let mut client = DXLinkClient::new(&tokens.streamer_url, &tokens.token);

    info!("Connecting to DXLink server: {}", tokens.streamer_url);
    let events = client.connect().await.map_err(|e| {
        // Through From, so an authentication refusal arrives as Auth and the
        // policy can tell it apart from a socket that dropped.
        let error: TastyTradeError = e.into();
        error
    })?;

    let channel_id = client
        .create_feed_channel("AUTO")
        .await
        .map_err(TastyTradeError::from)?;
    info!("DXLink channel created: {}", channel_id);

    // Deliberately no `setup_feed` here. The channel is configured when the
    // first subscription says what it wants, and reconfigured whenever a later
    // one wants more. Setting it up at connect time meant hardcoding a list —
    // it was Quote, Trade and Greeks — so a subscription asking for candles
    // was accepted locally and then never delivered anything, which is
    // indistinguishable from a quiet market.
    Ok(LiveConnection {
        client,
        channel_id,
        events,
    })
}

/// Configures the channel for `wanted`, if it is not already.
///
/// dxlink refuses a subscription to an event type the channel has no validated
/// configuration for, and `setup_feed` replaces the configuration rather than
/// adding to it — so this always sends the union, never just the new types.
///
/// # Errors
///
/// [`TastyTradeError::Connection`] when the socket is gone, so the caller can
/// tell that apart from a configuration the venue simply refused.
async fn ensure_configured(
    client: &mut DXLinkClient,
    channel_id: u32,
    configured: &mut BTreeSet<EventKind>,
    wanted: &BTreeSet<EventKind>,
) -> TastyResult<()> {
    if wanted.is_subset(configured) {
        return Ok(());
    }

    let union: BTreeSet<EventKind> = configured.union(wanted).copied().collect();
    let types: Vec<EventType> = union.iter().copied().map(feed_event_type).collect();

    debug!(
        "Configuring feed channel {channel_id} for {} event type(s)",
        types.len()
    );

    match client.setup_feed(channel_id, &types).await {
        Ok(()) => {
            *configured = union;
            Ok(())
        }
        Err(e) => {
            let lost = is_connection_lost(&e);
            let message = format!("the venue refused the feed configuration: {e}");
            Err(if lost {
                TastyTradeError::Connection(message)
            } else {
                TastyTradeError::Streaming(message)
            })
        }
    }
}

/// Why a connection stopped being used.
enum Ended {
    /// The owner dropped the streamer, or the command channel closed.
    Owner,
    /// A write failed in a way that says the socket is gone.
    ConnectionLost,
}

/// Forwards market events to the subscriptions registered for their symbol.
///
/// `saw_event` is the reconnect milestone: an event that actually arrived is
/// evidence the feed works, which a successful handshake is not. dxlink's
/// `subscribe` returns as soon as the write succeeds — the venue does not
/// acknowledge it — so resetting the attempt budget on a subscribe would reset
/// it on a connection that accepts the socket and then sends nothing, which is
/// the accept-then-reject loop the policy exists to bound.
/// Returns when the stream closes, which dxlink does when the session ends.
/// The supervisor watches this task for exactly that reason.
async fn forward_events(
    mut events: mpsc::Receiver<MarketEvent>,
    routing: Arc<RwLock<EventRouting>>,
    progress: CandleProgress,
    saw_event: Arc<AtomicBool>,
) {
    while let Some(event) = events.recv().await {
        // Anything arriving is evidence the feed works, whether or not this
        // crate models it. That is what the milestone is for.
        saw_event.store(true, Ordering::Relaxed);

        let Some(symbol) = event_symbol(&event) else {
            continue;
        };
        let symbol = symbol.to_string();
        let kind = event_kind(&event);

        let routing = routing.read().await;
        let Some(sub_ids) = routing.routes.get(&(symbol.clone(), kind)) else {
            debug!("No subscription registered for {kind} on {symbol}");
            continue;
        };

        // Delivery is charged per subscription: one consumer falling behind
        // must not decide where another one resumes from.
        for sub_id in sub_ids {
            let Some(subscribers) = routing.senders.get(sub_id) else {
                continue;
            };

            let mut delivered = false;
            let mut dropped = 0usize;
            for subscriber in subscribers {
                // A consumer that is not keeping up loses events rather than
                // stalling everyone else's. What changed is that losing them
                // is now countable and, for candles, recoverable.
                match subscriber.events.try_send(event.clone()) {
                    Ok(()) => delivered = true,
                    Err(_) => {
                        subscriber.lagged.fetch_add(1, Ordering::Relaxed);
                        dropped += 1;
                    }
                }
            }

            if dropped > 0 {
                // The symbol and the type only — market data never travels
                // with the warning.
                warn!(
                    "A consumer fell behind: dropped {kind} for {symbol} on {dropped} \
                     channel(s) of subscription {sub_id}"
                );
            }

            if let MarketEvent::Candle(candle) = &event {
                record_bar(
                    &progress,
                    *sub_id,
                    &symbol,
                    candle.time,
                    delivered && dropped == 0,
                );
            }
        }
    }
}

/// Moves a subscription's resume point, or marks that it cannot move.
///
/// The rule in one place because it is the subtle part. A bar that reached the
/// consumer with nothing missing before it advances `through`. A bar that was
/// dropped — for this consumer, on any of its channels — sets `gap`, and from
/// then on nothing advances until a replay clears it.
///
/// The alternative, taking the maximum of what was delivered, reads correctly
/// and is wrong: bar *n* dropped and bar *n+1* delivered moves the resume point
/// past *n*, so the reconnect never asks for it again and the series has a hole
/// nothing downstream can see.
fn record_bar(progress: &CandleProgress, sub_id: u32, symbol: &str, time: i64, complete: bool) {
    let mut seen = progress.lock().unwrap_or_else(|p| p.into_inner());
    let resume = seen
        .entry((sub_id, symbol.to_string()))
        .or_insert(CandleResume {
            through: time,
            gap: false,
        });

    if !complete {
        resume.gap = true;
        return;
    }
    if !resume.gap {
        resume.through = resume.through.max(time);
    }
}

/// Re-subscribes every symbol the subscriptions still hold.
///
/// Returns whether all of them landed. A partial replay is reported as a
/// failure, because a subscription silently missing half its symbols is worse
/// than one more reconnect.
async fn replay(
    client: &mut DXLinkClient,
    channel_id: u32,
    registry: &Registry,
    progress: &CandleProgress,
    configured: &mut BTreeSet<EventKind>,
) -> bool {
    let pending = pending_replay(registry, progress);

    if pending.is_empty() {
        return true;
    }

    // The new channel has to be configured before the venue will accept any
    // of this, and for exactly the event types the surviving subscriptions
    // hold — not for a fixed three, and not for all eleven.
    let wanted: BTreeSet<EventKind> = registry
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .values()
        .flat_map(|record| record.kinds.iter().copied())
        .collect();
    if let Err(e) = ensure_configured(client, channel_id, configured, &wanted).await {
        warn!("Could not configure the feed channel after reconnecting: {e}");
        return false;
    }

    debug!(
        "Restoring {} subscription(s) after a reconnect",
        pending.len()
    );

    // The replay is what refills a gap, so the flag that stopped a resume
    // point advancing is cleared as the request goes out. Leaving it set would
    // freeze every future reconnect at the same bar.
    {
        let mut seen = progress.lock().unwrap_or_else(|p| p.into_inner());
        for resume in seen.values_mut() {
            resume.gap = false;
        }
    }

    for (sub_id, requests) in pending {
        if let Err(e) = client.subscribe(channel_id, requests).await {
            warn!("Could not restore subscription {sub_id}: {e}");
            return false;
        }
    }
    true
}

/// What a replay would send, per subscription.
///
/// Separate from sending it because this is the part worth pinning: a
/// subscription that was closed, or one whose subscribe the venue refused,
/// must not come back on the next connection.
///
/// The lock is released before the caller awaits anything — a
/// `std::sync::Mutex` guard must not be held across an await.
fn pending_replay(
    registry: &Registry,
    progress: &CandleProgress,
) -> Vec<(u32, Vec<FeedSubscription>)> {
    let seen = progress.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let registry = registry.lock().unwrap_or_else(|p| p.into_inner());
    registry
        .iter()
        .map(|(sub_id, record)| {
            let targets: Vec<FeedTarget> = targets_of(&record.targets)
                .iter()
                .cloned()
                .map(|target| resume_from(*sub_id, target, &seen))
                .collect();
            (*sub_id, feed_subscriptions(&targets))
        })
        .filter(|(_, requests)| !requests.is_empty())
        .collect()
}

/// Where a candle subscription should pick up after a reconnect.
///
/// Replaying the original `from_time` would re-send every bar the consumer has
/// already been given — for one-minute candles over a day that is about 1440
/// events per symbol, duplicated on every reconnect, and a reconnect loop
/// multiplies it. So the replay starts from one millisecond past the last bar
/// delivered **contiguously**, and falls back to the original start when none
/// has been.
///
/// Contiguously is the word that matters. A subscription that dropped a bar
/// stops advancing its resume point, so the replay comes back from before the
/// gap and refills it — at the cost of duplicates after that point, which is
/// the safe direction: a duplicate bar is visible to a consumer and a missing
/// one is not.
///
/// Deliberately not "now": a drop that lasted a minute would leave a hole for
/// exactly the same reason.
fn resume_from(
    sub_id: u32,
    mut target: FeedTarget,
    seen: &HashMap<(u32, String), CandleResume>,
) -> FeedTarget {
    if target.kind != EventKind::Candle {
        return target;
    }
    if let Some(resume) = seen.get(&(sub_id, target.symbol.clone())) {
        target.from_time = Some(match target.from_time {
            Some(original) => original.max(resume.through.saturating_add(1)),
            None => resume.through.saturating_add(1),
        });
    }
    target
}

/// The symbol an event is about.
///
/// Every one of the eleven types the feed models carries an `eventSymbol`, so
/// this always answers. It stays an `Option` because the alternative is an
/// unwrap on a value the compiler cannot prove.
///
/// For a candle the symbol carries its period — `AAPL{=5m}` — which is what
/// keeps two periods of one underlying from delivering into each other.
fn event_symbol(event: &MarketEvent) -> Option<&str> {
    Some(match event {
        MarketEvent::Quote(quote) => &quote.event_symbol,
        MarketEvent::Trade(trade) => &trade.event_symbol,
        MarketEvent::TradeETH(trade) => &trade.event_symbol,
        MarketEvent::Greeks(greeks) => &greeks.event_symbol,
        MarketEvent::Candle(candle) => &candle.event_symbol,
        MarketEvent::Summary(summary) => &summary.event_symbol,
        MarketEvent::TimeAndSale(sale) => &sale.event_symbol,
        MarketEvent::Profile(profile) => &profile.event_symbol,
        MarketEvent::Underlying(underlying) => &underlying.event_symbol,
        MarketEvent::TheoPrice(theo) => &theo.event_symbol,
        MarketEvent::Series(series) => &series.event_symbol,
    })
}

/// Which of the eleven types an event is.
///
/// Exhaustive without a wildcard on purpose. Upstream `MarketEvent` is **not**
/// `#[non_exhaustive]`, so a twelfth variant breaks the build here — which is
/// the moment to decide whether this crate models it, rather than discovering
/// months later that its events were being dropped. That tripwire is what
/// produced this change: `0.3.1` added `TradeETH` and `Series` and the build
/// stopped compiling.
///
/// If upstream ever does add `#[non_exhaustive]`, replace this with a test
/// over the full type list rather than losing it to a `_ =>` arm.
fn event_kind(event: &MarketEvent) -> EventKind {
    match event {
        MarketEvent::Quote(_) => EventKind::Quote,
        MarketEvent::Trade(_) => EventKind::Trade,
        MarketEvent::TradeETH(_) => EventKind::TradeEth,
        MarketEvent::Greeks(_) => EventKind::Greeks,
        MarketEvent::Candle(_) => EventKind::Candle,
        MarketEvent::Summary(_) => EventKind::Summary,
        MarketEvent::TimeAndSale(_) => EventKind::TimeAndSale,
        MarketEvent::Profile(_) => EventKind::Profile,
        MarketEvent::Underlying(_) => EventKind::Underlying,
        MarketEvent::TheoPrice(_) => EventKind::TheoPrice,
        MarketEvent::Series(_) => EventKind::Series,
    }
}

/// The dxlink event type a kind subscribes as.
fn feed_event_type(kind: EventKind) -> EventType {
    match kind {
        EventKind::Quote => EventType::Quote,
        EventKind::Trade => EventType::Trade,
        EventKind::TradeEth => EventType::TradeETH,
        EventKind::Greeks => EventType::Greeks,
        EventKind::Candle => EventType::Candle,
        EventKind::Summary => EventType::Summary,
        EventKind::TimeAndSale => EventType::TimeAndSale,
        EventKind::Profile => EventType::Profile,
        EventKind::Underlying => EventType::Underlying,
        EventKind::TheoPrice => EventType::TheoPrice,
        EventKind::Series => EventType::Series,
    }
}

/// Records that `sub_id` wants these `(symbol, event type)` pairs.
///
/// Called before the subscribe is written, so no event can arrive for a route
/// that does not exist yet.
async fn record_routes(
    routing: &Arc<RwLock<EventRouting>>,
    sub_id: u32,
    subscriptions: &[FeedSubscription],
) {
    let mut routing = routing.write().await;
    for route in routes_of(subscriptions) {
        routing.routes.entry(route).or_default().insert(sub_id);
    }
}

/// Takes those routes back.
///
/// Used when the venue refuses the subscribe and when an unsubscribe lands. A
/// refused subscribe that keeps its route means the subscription receives
/// events for a symbol it was told it does not have, as soon as anybody else
/// subscribes to that symbol — and `add_symbols` has already given up its
/// reservation by then, so nothing else would ever clean it up.
async fn forget_routes(
    routing: &Arc<RwLock<EventRouting>>,
    sub_id: u32,
    subscriptions: &[FeedSubscription],
) {
    let mut routing = routing.write().await;
    for route in routes_of(subscriptions) {
        if let Some(subs) = routing.routes.get_mut(&route) {
            subs.remove(&sub_id);
            if subs.is_empty() {
                routing.routes.remove(&route);
            }
        }
    }
}

/// The routes a set of subscription requests covers.
///
/// A request whose event type this crate does not model has no route: nothing
/// could deliver it, and inventing a key would leave an entry nothing ever
/// removes.
fn routes_of(subscriptions: &[FeedSubscription]) -> Vec<(String, EventKind)> {
    subscriptions
        .iter()
        .filter_map(|sub| {
            EventKind::ALL
                .iter()
                .find(|kind| kind.wire_name() == sub.event_type)
                .map(|kind| (sub.symbol.clone(), *kind))
        })
        .collect()
}

/// Runs one connection until it is lost or the owner goes away.
#[allow(clippy::too_many_arguments)]
async fn run_connection(
    client: &mut DXLinkClient,
    channel_id: u32,
    commands: &mut mpsc::Receiver<DXLinkCommand>,
    shutdown: &mut oneshot::Receiver<()>,
    forwarder: &mut tokio::task::JoinHandle<()>,
    routing: &Arc<RwLock<EventRouting>>,
    configured: &mut BTreeSet<EventKind>,
) -> Ended {
    loop {
        let cmd = tokio::select! {
            biased;
            _ = &mut *shutdown => {
                debug!("Quote streamer owner dropped, disconnecting");
                return Ended::Owner;
            }
            // The event stream closing is dxlink saying the session is over.
            // It is the only signal that arrives without this client writing
            // anything, so it is what makes a drop visible to a consumer that
            // only reads — the ordinary shape of a market-data client, and the
            // case a write-failure check can never see.
            joined = &mut *forwarder => {
                match joined {
                    Ok(()) => debug!("The DXLink event stream closed; the session is over"),
                    // A forwarder that died is not a venue problem, and
                    // reconnecting will not fix it: the next one runs the same
                    // code over the same routing. Saying so is the difference
                    // between a diagnosable bug and a stream that quietly
                    // reconnects forever.
                    //
                    // Classification only. `JoinError`'s Display renders the
                    // panic payload, and a panic message can quote whatever
                    // the task was holding — here, market data.
                    Err(e) => warn!(
                        "The event forwarding task ended abnormally (panicked: {}, cancelled: {})",
                        e.is_panic(),
                        e.is_cancelled()
                    ),
                }
                return Ended::ConnectionLost;
            }
            cmd = commands.recv() => match cmd {
                Some(cmd) => cmd,
                None => return Ended::Owner,
            },
        };

        match cmd {
            // The channel id in the command is the one the handle was built
            // with. After a reconnect that number is stale, and only this loop
            // knows the live one.
            DXLinkCommand::Subscribe(subscriptions, kinds, sub_id, ack) => {
                // The venue refuses a subscription to an event type the
                // channel was not configured for, and only this loop knows
                // what the live channel is configured for. Nothing is
                // hardcoded: the channel ends up carrying exactly the types
                // the subscriptions asked for, and nothing else.
                if let Err(e) = ensure_configured(client, channel_id, configured, &kinds).await {
                    let lost = matches!(&e, TastyTradeError::Connection(_));
                    answer(ack, Err(e));
                    if lost {
                        return Ended::ConnectionLost;
                    }
                    continue;
                }

                record_routes(routing, sub_id, &subscriptions).await;

                match client.subscribe(channel_id, subscriptions.clone()).await {
                    Ok(()) => answer(ack, Ok(())),
                    Err(e) => {
                        let lost = is_connection_lost(&e);
                        error!("Error subscribing to symbols: {}", e);

                        // The route was recorded before the write, so a
                        // refused write has to take it back.
                        forget_routes(routing, sub_id, &subscriptions).await;

                        answer(
                            ack,
                            Err(TastyTradeError::Streaming(format!(
                                "the venue refused the subscription: {e}"
                            ))),
                        );
                        if lost {
                            return Ended::ConnectionLost;
                        }
                    }
                }
            }
            DXLinkCommand::Unsubscribe(subscriptions, sub_id, ack) => {
                // The venue is told first. Dropping the route before knowing
                // the unsubscribe landed leaves a subscription running with
                // nowhere to deliver, and the local state that could have
                // retried it already gone.
                let outcome = client.unsubscribe(channel_id, subscriptions.clone()).await;

                if outcome.is_ok() {
                    forget_routes(routing, sub_id, &subscriptions).await;
                }

                match outcome {
                    Ok(()) => answer(ack, Ok(())),
                    Err(e) => {
                        let lost = is_connection_lost(&e);
                        error!("Error unsubscribing from symbols: {}", e);
                        answer(
                            ack,
                            Err(TastyTradeError::Streaming(format!(
                                "the venue refused the unsubscribe: {e}"
                            ))),
                        );
                        if lost {
                            return Ended::ConnectionLost;
                        }
                    }
                }
            }
            DXLinkCommand::AddEventSender(subscription_id, sender) => {
                let mut routing = routing.write().await;
                routing
                    .senders
                    .entry(subscription_id)
                    .or_default()
                    .push(sender);
                debug!("Added event sender for subscription {}", subscription_id);
            }
            DXLinkCommand::RemoveEventSender(subscription_id) => {
                let mut routing = routing.write().await;
                routing.senders.remove(&subscription_id);
                routing.routes.retain(|_, subs| {
                    subs.remove(&subscription_id);
                    !subs.is_empty()
                });
                debug!("Removed event senders for subscription {}", subscription_id);
            }
        }
    }
}

/// Whether a dxlink failure means the socket is gone rather than the request
/// being wrong.
///
/// A refused subscription is the venue disagreeing with one request; a dead
/// socket makes every later request pointless. Only the second is worth
/// reconnecting for.
///
/// The second of two drop signals, and the faster one when this client is
/// writing: a failed write says so immediately, where the closing event stream
/// says so when dxlink's reader notices. A consumer that only reads has just
/// the stream, which is why both exist.
fn is_connection_lost(error: &dxlink::DXLinkError) -> bool {
    matches!(
        error,
        dxlink::DXLinkError::Connection(_) | dxlink::DXLinkError::WebSocket(_)
    )
}

/// Records why no further attempts will be made.
async fn terminal(state: &Arc<RwLock<ConnectionState>>, reason: String) {
    warn!("Quote stream gave up: {reason}");
    *state.write().await = ConnectionState::Disconnected { reason };
}

/// Waits out the backoff for the next attempt.
///
/// Returns false when the policy says to stop, or when the streamer was
/// dropped while waiting.
async fn schedule(
    policy: &BackoffPolicy,
    attempt: &mut u32,
    state: &Arc<RwLock<ConnectionState>>,
    shutdown: &mut oneshot::Receiver<()>,
) -> bool {
    *attempt = attempt.saturating_add(1);

    // Jitter source. A clock read is enough entropy to stop a fleet of clients
    // synchronising on the same venue restart, and it costs no dependency.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);

    let Some(delay) = policy.delay_for(*attempt, nanos) else {
        terminal(state, format!("gave up after {} attempts", *attempt - 1)).await;
        return false;
    };

    debug!("Quote stream reconnecting, attempt {attempt} in {delay:?}");
    *state.write().await = ConnectionState::Reconnecting {
        attempt: *attempt,
        delay,
    };

    // Cancellable: a caller who drops the streamer should not wait out a
    // thirty-second backoff for a task nobody is listening to.
    tokio::select! {
        _ = &mut *shutdown => false,
        _ = tokio::time::sleep(delay) => true,
    }
}

/// Owns the connection for the streamer's whole life, replacing it when lost.
#[allow(clippy::too_many_arguments)]
async fn supervise(
    tasty: TastyTrade,
    policy: BackoffPolicy,
    first: LiveConnection,
    mut commands: mpsc::Receiver<DXLinkCommand>,
    mut shutdown: oneshot::Receiver<()>,
    routing: Arc<RwLock<EventRouting>>,
    registry: Registry,
    state: Arc<RwLock<ConnectionState>>,
    progress: CandleProgress,
) {
    let mut attempt = 0u32;
    let mut next = Some(first);

    loop {
        let LiveConnection {
            mut client,
            channel_id,
            events,
        } = match next.take() {
            Some(connection) => connection,
            None => match connect_dxlink(&tasty).await {
                Ok(connection) => connection,
                Err(e) => {
                    // A rejected session is not a dropped socket: presenting
                    // the same credentials again will be refused again.
                    if !policy.should_retry(&e) {
                        terminal(&state, format!("reconnect refused: {e}")).await;
                        return;
                    }
                    if !schedule(&policy, &mut attempt, &state, &mut shutdown).await {
                        return;
                    }
                    continue;
                }
            },
        };

        // Forwarding starts before anything is subscribed, and `routing` is
        // the one that survived the reconnect, so an event that arrives the
        // instant the replay lands already has somewhere to go.
        let saw_event = Arc::new(AtomicBool::new(false));
        let mut forwarder = tokio::spawn(forward_events(
            events,
            routing.clone(),
            progress.clone(),
            saw_event.clone(),
        ));

        // A fresh channel is configured for nothing. What it ends up carrying
        // is the union of what the subscriptions ask for, and no more.
        let mut configured: BTreeSet<EventKind> = BTreeSet::new();

        let restored = replay(
            &mut client,
            channel_id,
            &registry,
            &progress,
            &mut configured,
        )
        .await;
        if restored {
            // Connected is claimed only once what was being watched is watched
            // again. Reporting it before restoration leaves a caller believing
            // they are receiving events they are not.
            *state.write().await = ConnectionState::Connected;
        }

        let ended = if restored {
            run_connection(
                &mut client,
                channel_id,
                &mut commands,
                &mut shutdown,
                &mut forwarder,
                &routing,
                &mut configured,
            )
            .await
        } else {
            warn!("Could not restore every subscription; reconnecting");
            Ended::ConnectionLost
        };

        forwarder.abort();

        // Why the session ended, when dxlink observed it rather than this
        // client. Logged, not carried into `ConnectionState`: the text comes
        // from whatever the socket reported, and the state value is public and
        // meant to be safe to show anywhere.
        if let Some(reason) = client.disconnect_reason() {
            debug!("DXLink reported the session ended: {reason}");
        }

        if let Err(e) = client.disconnect().await {
            debug!("Error disconnecting the previous DXLink client: {e}");
        }

        match ended {
            Ended::Owner => {
                *state.write().await = ConnectionState::Disconnected {
                    reason: "the streamer was dropped".to_string(),
                };
                debug!("DXLink supervisor terminated");
                return;
            }
            Ended::ConnectionLost => {
                // Reset only for a connection that actually delivered. A
                // handshake proves the venue accepts sockets, not that it
                // sends data; resetting on one lets an accepting-but-silent
                // venue loop forever at attempt one.
                if saw_event.load(Ordering::Relaxed) {
                    attempt = 0;
                }
                if !schedule(&policy, &mut attempt, &state, &mut shutdown).await {
                    return;
                }
            }
        }
    }
}

impl Drop for QuoteStreamer {
    fn drop(&mut self) {
        // No per-subscription unsubscribe loop here. Disconnecting ends every
        // subscription on the far side anyway, and close_sub is async now, so
        // it could not be awaited from Drop even if it were worth doing.

        // A oneshot send is synchronous, so this works outside a Tokio runtime
        // where tokio::spawn would panic, and it cannot be discarded by a full
        // command queue. Only the owner holds it, so nobody else can send it.
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::Symbol;

    /// The regression #19 exists for: create_sub stores one clone of the
    /// subscription and hands the caller another, so a per-copy Vec meant
    /// add_symbols updated the caller's while close_sub read the streamer's,
    /// which stayed empty. Unsubscribes were derived from that empty list and
    /// never sent, leaving the subscription alive on the venue.
    #[tokio::test]
    async fn both_copies_of_a_subscription_see_the_same_symbols() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let loop_handle = spawn_command_loop(rx, || Ok(()));

        let sub = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        sub.add_symbols(&[Symbol::from("AAPL"), Symbol::from("MSFT")])
            .await
            .expect("subscribing succeeds");

        // The streamer's own copy sees them, which is what close_sub reads.
        {
            let stored = targets_of(
                &streamer
                    .subscription_map
                    .get(&sub.id)
                    .expect("the streamer kept a copy")
                    .targets,
            );
            assert_eq!(stored.len(), 2, "the streamer's copy must see the symbols");
            assert!(stored.iter().any(|target| target.symbol == "AAPL"));
        }

        // Both the streamer and the subscription hold a command sender, so
        // the loop only ends when both are gone. That shared ownership is the
        // same property that made shutdown-by-command-queue wrong.
        drop(sub);
        drop(streamer);
        let sent = loop_handle.await.expect("the stand-in loop finishes");
        assert!(sent.contains(&"AAPL".to_string()));
        assert!(sent.contains(&"MSFT".to_string()));
    }

    /// Reaching the command queue is not the venue accepting. A refusal must
    /// leave nothing recorded, or the symbol is unsubscribed later as though
    /// it had been subscribed.
    #[tokio::test]
    async fn a_refused_subscription_records_nothing() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let _loop_handle = spawn_command_loop(rx, || {
            Err(TastyTradeError::Streaming("venue said no".to_string()))
        });

        let sub = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        let error = sub
            .add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect_err("a refused subscription is not a success");

        // The venue's own answer reaches the caller rather than being
        // flattened into a generic failure.
        assert!(format!("{error}").contains("venue said no"), "{error}");
        assert!(
            targets_of(&sub.targets).is_empty(),
            "a refused symbol must not stay reserved"
        );
    }

    /// A set, so asking twice subscribes once.
    #[tokio::test]
    async fn a_repeated_symbol_is_not_subscribed_twice() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let loop_handle = spawn_command_loop(rx, || Ok(()));

        let sub = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        sub.add_symbols(&[Symbol::from("AAPL")]).await.unwrap();
        sub.add_symbols(&[Symbol::from("AAPL")]).await.unwrap();

        drop(sub);
        drop(streamer);
        let sent = loop_handle.await.expect("the stand-in loop finishes");
        assert_eq!(
            sent.len(),
            1,
            "the second request had nothing new to say: {sent:?}"
        );
    }

    /// A symbol whose command never left must not be unsubscribed later as
    /// though it had.
    #[tokio::test]
    async fn a_failed_subscribe_records_nothing() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);

        let sub = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        drop(rx); // the command loop is gone

        sub.add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect_err("a closed streamer cannot subscribe");

        assert!(
            targets_of(&sub.targets).is_empty(),
            "nothing was subscribed, so nothing may be recorded"
        );
    }

    /// The regression this change exists for: a subscription used to hold a
    /// cloned `QuoteStreamer`, and that clone's `Drop` sent `Disconnect` on the
    /// shared command channel. Dropping a subscription therefore tore down the
    /// connection the real streamer was still using.
    ///
    /// A handle owns no connection, so dropping one has to leave the channel
    /// alive and silent.
    #[tokio::test]
    async fn dropping_a_handle_does_not_disconnect_anyone() {
        let (tx, mut rx) = mpsc::channel::<DXLinkCommand>(8);

        let handle = StreamerHandle {
            commands: Some(tx.clone()),
        };
        let second = handle.clone();

        drop(handle);
        drop(second);

        // Nothing was sent, and the channel is still usable by its owner.
        assert!(
            rx.try_recv().is_err(),
            "dropping a handle must not send a command"
        );
        tx.send(DXLinkCommand::RemoveEventSender(9))
            .await
            .expect("the owner's channel is still alive after handles are dropped");
        assert!(matches!(
            rx.recv().await,
            Some(DXLinkCommand::RemoveEventSender(9))
        ));
    }

    /// `Drop` must not need a Tokio runtime. `try_send` is synchronous;
    /// `tokio::spawn` would panic here, which is what this asserts by simply
    /// not panicking.
    /// Stands in for the command loop: drains commands, answers every one with
    /// `outcome`, and hands back what it saw. Without it `add_symbols` waits
    /// forever, which is the point — a subscription is not confirmed until
    /// something confirms it.
    pub(super) fn spawn_command_loop(
        mut rx: mpsc::Receiver<DXLinkCommand>,
        outcome: fn() -> TastyResult<()>,
    ) -> tokio::task::JoinHandle<Vec<String>> {
        tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    DXLinkCommand::Subscribe(requests, _, _, ack) => {
                        seen.extend(requests.into_iter().map(|r| r.symbol));
                        answer(ack, outcome());
                    }
                    DXLinkCommand::Unsubscribe(_, _, ack) => answer(ack, outcome()),
                    _ => {}
                }
            }
            seen
        })
    }

    pub(super) fn streamer_with(
        commands: mpsc::Sender<DXLinkCommand>,
        shutdown: oneshot::Sender<()>,
    ) -> QuoteStreamer {
        QuoteStreamer {
            shutdown: Some(shutdown),
            next_sub_id: 0,
            subscription_map: HashMap::new(),
            dxlink_command_tx: Some(commands),
            registry: Arc::new(Mutex::new(HashMap::new())),
            state: Arc::new(RwLock::new(ConnectionState::Connected)),
        }
    }

    #[test]
    fn dropping_the_owner_outside_a_runtime_does_not_panic() {
        let (tx, _rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        drop(streamer_with(tx, shutdown_tx));

        // Signalled synchronously rather than spawned, which is what a plain
        // #[test] proves by not panicking without a runtime.
        assert!(
            shutdown_rx.try_recv().is_ok(),
            "the owner must signal shutdown on drop"
        );
    }

    /// The command queue is bounded, so shutdown must not travel on it: a full
    /// queue would discard the disconnect and leave the client connected while
    /// its owner is gone.
    #[test]
    fn shutdown_survives_a_full_command_queue() {
        let (tx, _rx) = mpsc::channel::<DXLinkCommand>(1);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        tx.try_send(DXLinkCommand::RemoveEventSender(9))
            .expect("the first command fits");
        assert!(
            tx.try_send(DXLinkCommand::RemoveEventSender(9)).is_err(),
            "the queue must actually be full for this test to mean anything"
        );

        drop(streamer_with(tx, shutdown_tx));

        assert!(
            shutdown_rx.try_recv().is_ok(),
            "a full command queue must not be able to swallow the shutdown"
        );
    }
}

#[cfg(test)]
mod reconnect_tests {
    use super::lifecycle_tests::{spawn_command_loop, streamer_with};
    use super::*;
    use crate::Symbol;
    use dxlink::events::QuoteEvent;
    use std::time::Duration;

    fn quote(symbol: &str) -> MarketEvent {
        MarketEvent::Quote(QuoteEvent {
            event_type: "Quote".to_string(),
            event_symbol: symbol.to_string(),
            bid_price: 1.0,
            ask_price: 2.0,
            bid_size: 1.0,
            ask_size: 1.0,
        })
    }

    /// A target for a symbol under one event type, with no candle history.
    fn target(kind: EventKind, symbol: &str) -> FeedTarget {
        FeedTarget {
            kind,
            symbol: symbol.to_string(),
            from_time: None,
        }
    }

    /// No candle has been delivered yet.
    fn no_progress() -> CandleProgress {
        Arc::new(Mutex::new(HashMap::new()))
    }

    /// A consumer sink with its own loss counter.
    fn sink(events: mpsc::Sender<MarketEvent>) -> Subscriber {
        Subscriber {
            events,
            lagged: Arc::new(AtomicU64::new(0)),
        }
    }

    /// One candle for `symbol` at `time`.
    fn candle(symbol: &str, time: i64) -> MarketEvent {
        let MarketEvent::Candle(mut candle) = every_event_type(symbol)
            .into_iter()
            .find(|event| matches!(event, MarketEvent::Candle(_)))
            .expect("a candle")
        else {
            unreachable!("filtered on the variant")
        };
        candle.time = time;
        MarketEvent::Candle(candle)
    }

    /// One event of every type the feed models, all for `symbol`.
    fn every_event_type(symbol: &str) -> Vec<MarketEvent> {
        use dxlink::events::*;

        let sym = || symbol.to_string();
        vec![
            quote(symbol),
            MarketEvent::Trade(TradeEvent {
                event_type: "Trade".to_string(),
                event_symbol: sym(),
                price: 1.0,
                size: 1.0,
                day_volume: 10.0,
            }),
            MarketEvent::TradeETH(TradeETHEvent {
                event_type: "TradeETH".to_string(),
                event_symbol: sym(),
                event_time: 0,
                time: 0,
                time_nano_part: 0,
                sequence: 0,
                exchange_code: "Q".to_string(),
                price: 1.0,
                change: 0.0,
                size: 1.0,
                day_id: 0,
                day_volume: 0.0,
                day_turnover: 0.0,
                tick_direction: "Up".to_string(),
                extended_trading_hours: true,
            }),
            MarketEvent::Greeks(GreeksEvent {
                event_type: "Greeks".to_string(),
                event_symbol: sym(),
                delta: 0.5,
                gamma: 0.1,
                theta: -0.05,
                vega: 0.2,
                rho: 0.03,
                volatility: 0.25,
            }),
            MarketEvent::Candle(CandleEvent {
                event_type: "Candle".to_string(),
                event_symbol: sym(),
                event_time: 0,
                event_flags: 0,
                index: 0,
                time: 1_700_000_000_000,
                sequence: 0,
                count: 1,
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 100.0,
                vwap: 1.4,
                bid_volume: 50.0,
                ask_volume: 50.0,
                imp_volatility: 0.2,
                open_interest: 0.0,
            }),
            MarketEvent::Summary(SummaryEvent {
                event_type: "Summary".to_string(),
                event_symbol: sym(),
                event_time: 0,
                day_id: 0,
                day_open_price: 0.0,
                day_high_price: 0.0,
                day_low_price: 0.0,
                day_close_price: 0.0,
                day_close_price_type: "Final".to_string(),
                prev_day_id: 0,
                prev_day_close_price: 0.0,
                prev_day_close_price_type: "Final".to_string(),
                prev_day_volume: 0.0,
                open_interest: 0.0,
            }),
            MarketEvent::TimeAndSale(TimeAndSaleEvent {
                event_type: "TimeAndSale".to_string(),
                event_symbol: sym(),
                event_time: 0,
                event_flags: 0,
                index: 0,
                time: 0,
                time_nano_part: 0,
                sequence: 0,
                exchange_code: "Q".to_string(),
                price: 1.0,
                size: 1.0,
                bid_price: 0.9,
                ask_price: 1.1,
                exchange_sale_conditions: String::new(),
                trade_through_exempt: String::new(),
                aggressor_side: "Buy".to_string(),
                spread_leg: false,
                extended_trading_hours: false,
                valid_tick: true,
                sale_type: String::new(),
                buyer: String::new(),
                seller: String::new(),
            }),
            MarketEvent::Profile(ProfileEvent {
                event_type: "Profile".to_string(),
                event_symbol: sym(),
                event_time: 0,
                description: "Apple".to_string(),
                short_sale_restriction: "Inactive".to_string(),
                trading_status: "Active".to_string(),
                status_reason: String::new(),
                halt_start_time: 0,
                halt_end_time: 0,
                high_limit_price: 0.0,
                low_limit_price: 0.0,
                high_52_week_price: 0.0,
                low_52_week_price: 0.0,
                beta: 0.0,
                earnings_per_share: 0.0,
                dividend_frequency: 0.0,
                ex_dividend_amount: 0.0,
                ex_dividend_day_id: 0,
                shares: 0.0,
                free_float: 0.0,
            }),
            MarketEvent::Underlying(UnderlyingEvent {
                event_type: "Underlying".to_string(),
                event_symbol: sym(),
                event_time: 0,
                event_flags: 0,
                index: 0,
                time: 0,
                sequence: 0,
                volatility: 0.2,
                front_volatility: 0.21,
                back_volatility: 0.19,
                call_volume: 10.0,
                put_volume: 8.0,
                put_call_ratio: 0.8,
            }),
            MarketEvent::TheoPrice(TheoPriceEvent {
                event_type: "TheoPrice".to_string(),
                event_symbol: sym(),
                event_time: 0,
                event_flags: 0,
                index: 0,
                time: 0,
                sequence: 0,
                price: 1.0,
                underlying_price: 100.0,
                delta: 0.5,
                gamma: 0.1,
                dividend: 0.0,
                interest: 0.0,
            }),
            MarketEvent::Series(SeriesEvent {
                event_type: "Series".to_string(),
                event_symbol: sym(),
                event_time: 0,
                event_flags: 0,
                index: 0,
                time: 0,
                sequence: 0,
                expiration: 20_260_918,
                volatility: 0.2,
                call_volume: 10.0,
                put_volume: 8.0,
                put_call_ratio: 0.8,
                forward_price: 100.0,
                dividend: 0.0,
                interest: 0.0,
            }),
        ]
    }

    fn policy() -> BackoffPolicy {
        BackoffPolicy {
            initial: Duration::from_millis(10),
            max_delay: Duration::from_millis(40),
            max_attempts: Some(2),
            jitter: 0.0,
        }
    }

    /// A symbol is restored under exactly the event types it was subscribed
    /// with. Replaying a Quote-only subscription as Quote+Trade would start a
    /// stream the caller never asked for and cannot see.
    #[test]
    fn a_replay_asks_for_the_event_types_the_subscription_had() {
        let requests = feed_subscriptions(&[
            target(EventKind::Quote, "AAPL"),
            target(EventKind::Greeks, "AAPL"),
            target(EventKind::Quote, "MSFT"),
            target(EventKind::Greeks, "MSFT"),
        ]);

        assert_eq!(requests.len(), 4, "two symbols by two event types");
        let types: BTreeSet<&str> = requests.iter().map(|r| r.event_type.as_str()).collect();
        assert_eq!(
            types,
            BTreeSet::from(["Greeks", "Quote"]),
            "Trade was not asked for: {types:?}"
        );
    }

    /// The replay is derived from the confirmed symbols, so a subscription
    /// that never got any contributes nothing.
    #[test]
    fn a_subscription_with_no_confirmed_symbols_is_not_replayed() {
        let registry: Registry = Arc::new(Mutex::new(HashMap::new()));
        registry.lock().unwrap().insert(
            7,
            SubscriptionRecord {
                kinds: BTreeSet::from([EventKind::Quote]),
                targets: Arc::new(Mutex::new(BTreeSet::new())),
            },
        );

        assert!(
            pending_replay(&registry, &no_progress()).is_empty(),
            "a refused subscribe records nothing, so there is nothing to restore"
        );
    }

    /// A closed subscription must not come back on the next connection: the
    /// caller unsubscribed it, and a reconnect is not a reason to overrule
    /// that.
    #[tokio::test]
    async fn a_closed_subscription_is_not_replayed() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let _loop_handle = spawn_command_loop(rx, || Ok(()));

        let sub = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        sub.add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect("subscribing succeeds");

        let pending = pending_replay(&streamer.registry, &no_progress());
        assert_eq!(pending.len(), 1, "a live subscription is restored");
        assert_eq!(pending[0].1[0].symbol, "AAPL");

        streamer.close_sub(sub.id).await.expect("closing succeeds");

        assert!(
            pending_replay(&streamer.registry, &no_progress()).is_empty(),
            "a closed subscription must not be resubscribed by a reconnect"
        );
    }

    /// The routing registry outlives a connection, so an event arriving right
    /// after a reconnect already has somewhere to go. This drives the
    /// forwarding side of that: a route registered before the connection
    /// existed still delivers.
    #[tokio::test]
    async fn a_forwarded_event_reaches_the_subscription_registered_for_it() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let (sub_tx, mut sub_rx) = mpsc::channel::<MarketEvent>(4);
        {
            let mut routing = routing.write().await;
            routing.senders.insert(1, vec![sink(sub_tx)]);
            routing
                .routes
                .insert(("AAPL".to_string(), EventKind::Quote), HashSet::from([1]));
        }

        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let saw_event = Arc::new(AtomicBool::new(false));
        let forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            no_progress(),
            saw_event.clone(),
        ));

        events_tx
            .send(quote("AAPL"))
            .await
            .expect("the feed accepts");
        let received = sub_rx
            .recv()
            .await
            .expect("the subscription is delivered to");
        assert!(matches!(received, MarketEvent::Quote(q) if q.event_symbol == "AAPL"));

        // A symbol nobody is subscribed to is dropped rather than panicking or
        // being broadcast to everyone.
        events_tx
            .send(quote("TSLA"))
            .await
            .expect("the feed accepts");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), sub_rx.recv())
                .await
                .is_err(),
            "an unrouted symbol must not be delivered"
        );

        assert!(
            saw_event.load(Ordering::Relaxed),
            "an event that arrived is the milestone the backoff resets on"
        );
        forwarder.abort();
    }

    /// A subscription the venue refused must not keep its route. `add_symbols`
    /// gives up its reservation on failure, so nothing else would ever remove
    /// it, and the subscription would start receiving events for that symbol
    /// as soon as anybody else subscribed to it.
    #[tokio::test]
    async fn a_refused_subscribe_takes_its_route_back() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let wanted = feed_subscriptions(&[target(EventKind::Quote, "AAPL")]);

        // Two subscriptions ask for the same symbol; one of them is refused.
        record_routes(&routing, 3, &wanted).await;
        record_routes(&routing, 4, &wanted).await;
        forget_routes(&routing, 3, &wanted).await;

        let routes = routing.read().await;
        let subs = routes
            .routes
            .get(&("AAPL".to_string(), EventKind::Quote))
            .expect("the accepted subscription still holds the symbol");
        assert!(
            !subs.contains(&3),
            "a refused subscribe must not leave a route behind"
        );
        assert!(subs.contains(&4), "the accepted one keeps its route");
    }

    /// The symbol goes with the last subscription that wanted it, so an
    /// unrouted event is dropped rather than delivered to a stale id.
    #[tokio::test]
    async fn the_last_route_removed_takes_the_symbol_with_it() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let wanted = feed_subscriptions(&[target(EventKind::Quote, "AAPL")]);

        record_routes(&routing, 3, &wanted).await;
        forget_routes(&routing, 3, &wanted).await;

        assert!(
            routing.read().await.routes.is_empty(),
            "an empty set must not be left behind as a route"
        );
    }

    /// The whole point of #67. A consumer that only reads — the ordinary shape
    /// of a market-data client — never issues a command, so a write-failure
    /// check can never see its connection die. dxlink 0.3 closes the event
    /// stream when the session ends, and this is the supervisor noticing.
    #[tokio::test]
    async fn a_closed_event_stream_ends_the_connection_without_any_write() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let saw_event = Arc::new(AtomicBool::new(false));
        let mut forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            no_progress(),
            saw_event.clone(),
        ));

        // Never connected: the point is that nothing is written to it.
        let mut client = DXLinkClient::new("wss://127.0.0.1:1", "unused");
        let (_commands_tx, mut commands_rx) = mpsc::channel::<DXLinkCommand>(4);
        let (_shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // The session ends: dxlink drops its side of the event stream.
        drop(events_tx);

        let ended = tokio::time::timeout(
            Duration::from_secs(2),
            run_connection(
                &mut client,
                1,
                &mut commands_rx,
                &mut shutdown_rx,
                &mut forwarder,
                &routing,
                &mut BTreeSet::new(),
            ),
        )
        .await
        .expect("the closing stream must end the connection, not be waited on forever");

        assert!(
            matches!(ended, Ended::ConnectionLost),
            "a closed event stream is a lost connection"
        );
    }

    /// A forwarder that dies is not a venue problem, and the next connection
    /// runs the same code over the same routing. It still ends the connection,
    /// but it must not do so silently — that is the difference between a
    /// diagnosable bug and a stream that reconnects forever for no visible
    /// reason.
    #[tokio::test]
    async fn a_forwarder_that_panics_is_reported_rather_than_read_as_a_venue_drop() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let mut forwarder = tokio::spawn(async { panic!("the forwarder died") });

        let mut client = DXLinkClient::new("wss://127.0.0.1:1", "unused");
        let (_commands_tx, mut commands_rx) = mpsc::channel::<DXLinkCommand>(4);
        let (_shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        let ended = tokio::time::timeout(
            Duration::from_secs(2),
            run_connection(
                &mut client,
                1,
                &mut commands_rx,
                &mut shutdown_rx,
                &mut forwarder,
                &routing,
                &mut BTreeSet::new(),
            ),
        )
        .await
        .expect("a dead forwarder must end the connection, not be waited on");

        assert!(matches!(ended, Ended::ConnectionLost));
    }

    /// The owner still wins over a live connection: dropping the streamer must
    /// end the supervisor rather than wait for the venue to do something.
    #[tokio::test]
    async fn the_owner_still_takes_precedence_over_a_live_stream() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let (_events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let mut forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            no_progress(),
            Arc::new(AtomicBool::new(false)),
        ));

        let mut client = DXLinkClient::new("wss://127.0.0.1:1", "unused");
        let (_commands_tx, mut commands_rx) = mpsc::channel::<DXLinkCommand>(4);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        drop(shutdown_tx);

        let ended = tokio::time::timeout(
            Duration::from_secs(2),
            run_connection(
                &mut client,
                1,
                &mut commands_rx,
                &mut shutdown_rx,
                &mut forwarder,
                &routing,
                &mut BTreeSet::new(),
            ),
        )
        .await
        .expect("a dropped owner ends the connection promptly");

        assert!(matches!(ended, Ended::Owner));
    }

    /// Every one of the eleven types the feed models has to reach the caller.
    /// Eight of them used to be logged and dropped, which for `Candle` meant
    /// there was no route to historical bars anywhere in this crate, and for
    /// `TradeETH` no route to an extended-hours price.
    #[tokio::test]
    async fn all_eleven_event_types_convert_and_are_delivered() {
        let events = every_event_type("AAPL");
        assert_eq!(events.len(), EventKind::ALL.len());

        let (tx, rx) = mpsc::channel::<MarketEvent>(32);
        let (_unused_tx, event_receiver) = flume::unbounded();
        let mut subscription = QuoteSubscription {
            id: SubscriptionId(0),
            streamer: StreamerHandle { commands: None },
            kinds: EventKind::ALL.into_iter().collect(),
            event_receiver,
            dxlink_receiver: rx,
            targets: Arc::new(Mutex::new(BTreeSet::new())),
            lagged: Arc::new(AtomicU64::new(0)),
        };

        for event in &events {
            // Every variant knows its own symbol and its own kind.
            assert_eq!(event_symbol(event), Some("AAPL"), "{:?}", event_kind(event));
            tx.send(event.clone()).await.expect("the feed accepts");
        }

        let mut seen = BTreeSet::new();
        for _ in 0..events.len() {
            let event = tokio::time::timeout(Duration::from_secs(2), subscription.get_event())
                .await
                .expect("no modelled event may stall the reader")
                .expect("every modelled event is readable");
            assert_eq!(event.sym, "AAPL");
            seen.insert(event.data.kind());
        }

        assert_eq!(
            seen,
            EventKind::ALL.into_iter().collect::<BTreeSet<_>>(),
            "every event type must arrive as its own variant"
        );
    }

    /// The tripwire that produced this change. `MarketEvent` is not
    /// `#[non_exhaustive]`, so a twelfth variant breaks the build rather than
    /// being silently dropped — and this pins the count so the list cannot
    /// quietly shrink either.
    #[test]
    fn the_kinds_this_crate_routes_cover_every_variant_the_feed_models() {
        let kinds: BTreeSet<EventKind> = every_event_type("AAPL").iter().map(event_kind).collect();

        assert_eq!(kinds, EventKind::ALL.into_iter().collect::<BTreeSet<_>>());
    }

    /// The routing fix candles need. Two periods of one underlying are two
    /// streamer symbols, so a subscription watching five-minute bars must not
    /// be handed the hourly ones.
    #[tokio::test]
    async fn two_candle_periods_of_one_underlying_do_not_cross_deliver() {
        let five = CandlePeriod::minutes(5).expect("a period");
        let hour = CandlePeriod::hours(1).expect("a period");

        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let (five_tx, mut five_rx) = mpsc::channel::<MarketEvent>(4);
        let (hour_tx, mut hour_rx) = mpsc::channel::<MarketEvent>(4);
        {
            let mut routing = routing.write().await;
            routing.senders.insert(1, vec![sink(five_tx)]);
            routing.senders.insert(2, vec![sink(hour_tx)]);
        }

        record_routes(
            &routing,
            1,
            &feed_subscriptions(&[FeedTarget {
                kind: EventKind::Candle,
                symbol: five.streamer_symbol("AAPL"),
                from_time: Some(0),
            }]),
        )
        .await;
        record_routes(
            &routing,
            2,
            &feed_subscriptions(&[FeedTarget {
                kind: EventKind::Candle,
                symbol: hour.streamer_symbol("AAPL"),
                from_time: Some(0),
            }]),
        )
        .await;

        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let progress = no_progress();
        let forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            progress.clone(),
            Arc::new(AtomicBool::new(false)),
        ));

        let bar = every_event_type(&five.streamer_symbol("AAPL"))
            .into_iter()
            .find(|event| matches!(event, MarketEvent::Candle(_)))
            .expect("a candle");
        events_tx.send(bar).await.expect("the feed accepts");

        let delivered = five_rx.recv().await.expect("the five-minute subscription");
        assert!(matches!(delivered, MarketEvent::Candle(_)));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), hour_rx.recv())
                .await
                .is_err(),
            "the hourly subscription must not see a five-minute bar"
        );

        // And the bar was recorded, which is what a reconnect resumes from.
        let seen = progress.lock().expect("not poisoned in tests").clone();
        assert_eq!(
            seen.get(&(1, five.streamer_symbol("AAPL")))
                .map(|r| r.through),
            Some(1_700_000_000_000)
        );

        forwarder.abort();
    }

    /// Routing is keyed by symbol **and** event type, so a subscription that
    /// asked for quotes on AAPL does not receive the trade prints another
    /// subscription asked for.
    #[tokio::test]
    async fn a_subscription_only_receives_the_event_types_it_asked_for() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        let (quotes_tx, mut quotes_rx) = mpsc::channel::<MarketEvent>(4);
        routing
            .write()
            .await
            .senders
            .insert(1, vec![sink(quotes_tx)]);

        record_routes(
            &routing,
            1,
            &feed_subscriptions(&[target(EventKind::Quote, "AAPL")]),
        )
        .await;

        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            no_progress(),
            Arc::new(AtomicBool::new(false)),
        ));

        let trade = every_event_type("AAPL")
            .into_iter()
            .find(|event| matches!(event, MarketEvent::Trade(_)))
            .expect("a trade");
        events_tx.send(trade).await.expect("the feed accepts");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), quotes_rx.recv())
                .await
                .is_err(),
            "a Quote subscription must not receive Trade prints"
        );

        events_tx
            .send(quote("AAPL"))
            .await
            .expect("the feed accepts");
        assert!(quotes_rx.recv().await.is_some(), "the quote still arrives");

        forwarder.abort();
    }

    /// A reconnect must not re-send a day of bars the consumer already has,
    /// and must not skip the ones it missed either.
    #[test]
    fn a_candle_replay_resumes_after_the_last_contiguous_bar() {
        let original = FeedTarget {
            kind: EventKind::Candle,
            symbol: "AAPL{=5m}".to_string(),
            from_time: Some(1_000),
        };
        let seen = HashMap::from([(
            (1u32, "AAPL{=5m}".to_string()),
            CandleResume {
                through: 5_000,
                gap: false,
            },
        )]);

        assert_eq!(
            resume_from(1, original.clone(), &seen).from_time,
            Some(5_001),
            "the replay picks up one millisecond past the last bar delivered"
        );

        // Nothing delivered yet: the caller's own start still stands.
        assert_eq!(
            resume_from(1, original.clone(), &HashMap::new()).from_time,
            Some(1_000)
        );

        // Another subscription's progress is not this one's. A consumer that
        // kept up must not decide where a consumer that did not resumes from.
        assert_eq!(
            resume_from(2, original.clone(), &seen).from_time,
            Some(1_000)
        );

        // A different period is a different series.
        let other = FeedTarget {
            symbol: "AAPL{=1h}".to_string(),
            ..original.clone()
        };
        assert_eq!(resume_from(1, other, &seen).from_time, Some(1_000));

        // Only candles carry one at all.
        assert_eq!(
            resume_from(1, target(EventKind::Quote, "AAPL"), &seen).from_time,
            None
        );
    }

    /// `from_time` reaches the wire. It was already on `FeedSubscription` and
    /// always `None`, which is why there was no way to ask for history.
    #[test]
    fn a_candle_request_carries_its_history_start() {
        let requests = feed_subscriptions(&[FeedTarget {
            kind: EventKind::Candle,
            symbol: "AAPL{=5m}".to_string(),
            from_time: Some(1_700_000_000_000),
        }]);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].event_type, "Candle");
        assert_eq!(requests[0].symbol, "AAPL{=5m}");
        assert_eq!(requests[0].from_time, Some(1_700_000_000_000));
    }

    /// A candle needs a period and a start time, so the two subscription
    /// calls are not interchangeable and each says so rather than quietly
    /// subscribing to something that cannot arrive.
    #[tokio::test]
    async fn candles_and_bare_symbols_are_not_interchangeable() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let _loop_handle = spawn_command_loop(rx, || Ok(()));

        let quotes = streamer
            .create_sub([EventKind::Quote])
            .await
            .expect("the streamer is open");
        let error = quotes
            .add_candles(
                &[Symbol::from("AAPL")],
                CandlePeriod::minutes(5).expect("a period"),
                DateTime::from_timestamp(1_700_000_000, 0).expect("a timestamp"),
            )
            .await
            .expect_err("the channel is not configured for candles");
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "{error:?}"
        );

        let candles = streamer
            .create_sub([EventKind::Candle])
            .await
            .expect("the streamer is open");
        let error = candles
            .add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect_err("a bare symbol has no period and no start time");
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "{error:?}"
        );
    }

    /// The candle path end to end: the symbol the venue is told carries the
    /// period, and that is what the subscription records.
    #[tokio::test]
    async fn a_candle_subscription_is_recorded_under_its_period_symbol() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        let loop_handle = spawn_command_loop(rx, || Ok(()));

        let sub = streamer
            .create_sub([EventKind::Candle])
            .await
            .expect("the streamer is open");
        sub.add_candles(
            &[Symbol::from("AAPL")],
            CandlePeriod::minutes(5).expect("a period"),
            DateTime::from_timestamp(1_700_000_000, 0).expect("a timestamp"),
        )
        .await
        .expect("subscribing succeeds");

        assert_eq!(
            sub.subscribed(),
            vec![("AAPL{=5m}".to_string(), EventKind::Candle)]
        );

        drop(sub);
        drop(streamer);
        let sent = loop_handle.await.expect("the stand-in loop finishes");
        assert_eq!(sent, vec!["AAPL{=5m}".to_string()]);
    }

    /// The ordering bug candles made acute. `create_sub` used to register the
    /// returned subscription's event route from a detached `tokio::spawn`, so
    /// a caller that subscribed immediately could have the subscribe reach the
    /// command loop first — and a candle history arrives at once, so the first
    /// bars were routed to a subscription the loop did not know about and
    /// dropped.
    ///
    /// Registration is now awaited, so it is on the queue before `create_sub`
    /// returns and therefore before anything the caller does next.
    #[tokio::test]
    async fn the_event_route_is_registered_before_a_subscription_can_be_used() {
        let (tx, mut rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);

        let sub = streamer
            .create_sub([EventKind::Candle])
            .await
            .expect("the streamer is open");

        // Already queued — nothing was left to a task that may or may not have
        // run.
        let mut registrations = 0;
        while let Ok(DXLinkCommand::AddEventSender(id, _)) = rx.try_recv() {
            assert_eq!(id, sub.id.0 as u32);
            registrations += 1;
        }
        assert_eq!(
            registrations, 1,
            "exactly one consumer is registered: the caller's. The streamer's own \
             copy used to register a second that nothing could ever read, so it \
             filled up and then charged a drop for every event afterwards"
        );
    }

    /// A closed streamer cannot route events, so handing back a subscription
    /// that can never receive anything would be the same silent failure in a
    /// different place.
    #[tokio::test]
    async fn creating_a_subscription_on_a_closed_streamer_fails() {
        let (tx, rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);
        drop(rx);

        let Err(error) = streamer.create_sub([EventKind::Quote]).await else {
            panic!("a closed streamer cannot register a route");
        };
        assert!(matches!(error, TastyTradeError::Streaming(_)), "{error:?}");
    }

    /// A bar that was dropped for a slow consumer must not advance the resume
    /// point: the reconnect would skip it permanently, and a hole in a price
    /// series is worse than a duplicate because nothing downstream can see it.
    ///
    /// The subtle half is *contiguity*. Taking the maximum of what was
    /// delivered reads correctly and is wrong: bar 1000 dropped and bar 3000
    /// delivered would move the resume point to 3000 and lose 2000 forever.
    #[tokio::test]
    async fn a_dropped_bar_freezes_the_resume_point_rather_than_being_stepped_over() {
        let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
        // Capacity one and nothing ever reads it: the first bar fits, the rest
        // are dropped.
        let (full_tx, _never_read) = mpsc::channel::<MarketEvent>(1);
        let lagged = Arc::new(AtomicU64::new(0));
        routing.write().await.senders.insert(
            1,
            vec![Subscriber {
                events: full_tx,
                lagged: lagged.clone(),
            }],
        );
        record_routes(
            &routing,
            1,
            &feed_subscriptions(&[FeedTarget {
                kind: EventKind::Candle,
                symbol: "AAPL{=5m}".to_string(),
                from_time: Some(0),
            }]),
        )
        .await;

        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(8);
        let progress = no_progress();
        let forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
            progress.clone(),
            Arc::new(AtomicBool::new(false)),
        ));

        for time in [1_000i64, 2_000, 3_000] {
            events_tx
                .send(candle("AAPL{=5m}", time))
                .await
                .expect("the feed accepts");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        let resume = progress
            .lock()
            .expect("not poisoned in tests")
            .get(&(1, "AAPL{=5m}".to_string()))
            .copied()
            .expect("the series was seen");

        assert_eq!(
            resume.through, 1_000,
            "only the bar that was actually delivered may be resumed past"
        );
        assert!(
            resume.gap,
            "a drop has to be remembered, or the next delivery steps over it"
        );

        // And the consumer can find out, which is the whole point: two of the
        // three bars never reached it.
        assert_eq!(lagged.load(Ordering::Relaxed), 2);

        forwarder.abort();
    }

    /// A gap is refilled by the replay, so the flag that froze the resume
    /// point has to be cleared when the request goes out — otherwise every
    /// future reconnect asks from the same bar forever.
    #[test]
    fn a_replay_resumes_from_before_the_gap() {
        let seen = HashMap::from([(
            (1u32, "AAPL{=5m}".to_string()),
            CandleResume {
                through: 1_000,
                gap: true,
            },
        )]);

        let target = FeedTarget {
            kind: EventKind::Candle,
            symbol: "AAPL{=5m}".to_string(),
            from_time: Some(0),
        };

        assert_eq!(
            resume_from(1, target, &seen).from_time,
            Some(1_001),
            "the replay comes back from before the gap and refills it"
        );
    }

    /// A subscription with no buffer would drop every event and then report
    /// itself as lagging, which is a worse way to learn about it.
    #[tokio::test]
    async fn a_subscription_with_no_buffer_is_refused() {
        let (tx, _rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);

        let Err(error) = streamer
            .create_sub_with_capacity([EventKind::Quote], 0)
            .await
        else {
            panic!("a zero-capacity subscription cannot deliver anything");
        };
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "{error:?}"
        );
    }

    /// The budget is bounded, and running out is reported rather than
    /// retried silently forever.
    #[tokio::test]
    async fn the_backoff_gives_up_and_says_why() {
        let state = Arc::new(RwLock::new(ConnectionState::Connected));
        let (_shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let mut attempt = 0u32;

        assert!(schedule(&policy(), &mut attempt, &state, &mut shutdown_rx).await);
        assert_eq!(
            *state.read().await,
            ConnectionState::Reconnecting {
                attempt: 1,
                delay: Duration::from_millis(10)
            }
        );

        assert!(schedule(&policy(), &mut attempt, &state, &mut shutdown_rx).await);
        assert!(
            !schedule(&policy(), &mut attempt, &state, &mut shutdown_rx).await,
            "one past the limit must stop"
        );

        let ConnectionState::Disconnected { reason } = state.read().await.clone() else {
            panic!("giving up must be terminal, not another retry");
        };
        assert!(reason.contains("2 attempts"), "{reason}");
    }

    /// Dropping the streamer during a backoff must end the supervisor then,
    /// not after the full delay: a caller who let go should not keep a task
    /// waiting thirty seconds for nobody.
    #[tokio::test]
    async fn a_backoff_is_interrupted_by_the_owner_going_away() {
        let state = Arc::new(RwLock::new(ConnectionState::Connected));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let slow = BackoffPolicy {
            initial: Duration::from_secs(30),
            max_attempts: None,
            ..policy()
        };
        let mut attempt = 0u32;

        drop(shutdown_tx);

        assert!(
            !schedule(&slow, &mut attempt, &state, &mut shutdown_rx).await,
            "a dropped owner ends the wait"
        );
    }

    /// A refused subscription is the venue disagreeing with one request; a
    /// dead socket makes every later request pointless. Reconnecting on the
    /// first would turn a bad symbol into a reconnect loop.
    #[test]
    fn only_a_dead_socket_counts_as_a_lost_connection() {
        assert!(is_connection_lost(&dxlink::DXLinkError::Connection(
            "closed".to_string()
        )));
        assert!(!is_connection_lost(&dxlink::DXLinkError::Protocol(
            "unknown symbol".to_string()
        )));
        assert!(!is_connection_lost(&dxlink::DXLinkError::Authentication(
            "token expired".to_string()
        )));
    }

    /// An authentication failure is not retried: the same token will be
    /// refused again, and the policy is what says so.
    #[test]
    fn a_rejected_session_is_not_worth_retrying() {
        let policy = BackoffPolicy::default();
        let refused: TastyTradeError = dxlink::DXLinkError::Authentication("nope".into()).into();

        assert!(!policy.should_retry(&refused), "{refused:?}");
        assert!(policy.should_retry(&TastyTradeError::Connection("dropped".into())));
    }
}
