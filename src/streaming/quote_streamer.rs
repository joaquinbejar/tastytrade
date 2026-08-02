// For quote_streamer.rs
use crate::TastyTrade;
use crate::streaming::reconnect::{BackoffPolicy, ConnectionState};
use crate::types::dxfeed;
use crate::{AsSymbol, Symbol, TastyResult, TastyTradeError};
use dxlink::{DXLinkClient, EventType, FeedSubscription, MarketEvent};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
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
    event_types: i32, // Keep for compatibility with existing code
    event_receiver: flume::Receiver<dxfeed::Event>, // Keep for compatibility
    dxlink_receiver: mpsc::Receiver<MarketEvent>, // New DXLink event receiver
    /// The symbols this subscription is actually subscribed to.
    ///
    /// Shared with the copy the streamer keeps in its `subscription_map`, and
    /// that sharing is the fix rather than an optimisation: `create_sub`
    /// stores one clone and hands the caller another, so a `Vec` on each
    /// meant `add_symbols` updated the caller's copy while `close_sub` read
    /// the streamer's, which stayed empty forever. Unsubscribes were
    /// therefore derived from an empty list and never sent, leaving the
    /// subscription alive on the venue.
    ///
    /// A set, so adding the same symbol twice subscribes once.
    symbols: Arc<Mutex<BTreeSet<Symbol>>>,
}

impl QuoteSubscription {
    /// Subscribes this subscription to `symbols`.
    ///
    /// Returns once the venue has accepted the subscription, not merely once
    /// the command was queued. Symbols already subscribed are skipped, so
    /// calling twice with the same symbol subscribes once.
    ///
    /// # Errors
    ///
    /// Fails when the streamer has no open channel, when it is closed, or when
    /// the venue refuses. On any of those the symbols are not recorded, so a
    /// later close does not try to unsubscribe something that was never
    /// subscribed.
    pub async fn add_symbols<S: AsSymbol>(&self, symbols: &[S]) -> TastyResult<()> {
        let requested: Vec<Symbol> = symbols.iter().map(|sym| sym.as_symbol()).collect();

        // Only symbols that are not already subscribed. Asking the venue twice
        // for the same symbol is at best wasted work and at worst a duplicate
        // stream.
        // Checked and reserved in one lock section. Filtering against the set
        // and inserting afterwards let two concurrent callers both see a
        // symbol as absent and both subscribe to it. Reserving here means the
        // second caller sees the first one's claim; a failure below removes
        // the reservation again.
        let symbols: Vec<Symbol> = {
            let mut known = symbols_of(&self.symbols);
            requested
                .into_iter()
                .filter(|sym| known.insert(sym.clone()))
                .collect()
        };

        let subscriptions = feed_subscriptions(&symbols, self.event_types);

        if subscriptions.is_empty() {
            return Ok(());
        }

        // Awaited rather than spawned. A detached task meant this returned
        // success before the command was even accepted, so a caller could not
        // tell a subscription that worked from one that never left, and it
        // panicked outright when called without a Tokio runtime.
        let sub_id = self.id.0 as u32;
        let Some(tx) = &self.streamer.commands else {
            return Err(TastyTradeError::Streaming(
                "the quote streamer has no command channel; reconnect before subscribing"
                    .to_string(),
            ));
        };

        let (ack, answered) = oneshot::channel();
        let queued = tx
            .send(DXLinkCommand::Subscribe(subscriptions, sub_id, Some(ack)))
            .await
            .map_err(|_| {
                TastyTradeError::Streaming(
                    "the quote streamer is closed; reconnect before subscribing".to_string(),
                )
            });

        // Reaching the command queue is not the venue accepting the
        // subscription: the loop can still be refused by DXLink. Wait for the
        // real answer, and give back the reservation if it is a refusal, so a
        // symbol that is not subscribed is never later unsubscribed as though
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
            let mut known = symbols_of(&self.symbols);
            for symbol in &symbols {
                known.remove(symbol);
            }
        }

        outcome
    }

    /// Receive one event from feed. Yields if there are no events.
    /// Compatible with previous interface
    pub async fn get_event(&mut self) -> Result<dxfeed::Event, flume::RecvError> {
        // Try to receive event from DXLink
        match self.dxlink_receiver.recv().await {
            Some(market_event) => {
                // Convert from DXLink MarketEvent to dxfeed Event
                match market_event {
                    MarketEvent::Quote(quote) => {
                        let symbol = quote.event_symbol;
                        let data = dxfeed::EventData::Quote(dxfeed::DxfQuoteT {
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
                        });
                        Ok(dxfeed::Event { sym: symbol, data })
                    }
                    MarketEvent::Trade(trade) => {
                        // Convert Trade to dxfeed format
                        let symbol = trade.event_symbol;
                        let data = dxfeed::EventData::Trade(dxfeed::DxfTradeT {
                            time: 0,
                            sequence: 0,
                            time_nanos: 0,
                            exchange_code: 0,
                            price: trade.price,
                            size: trade.size as i64,

                            tick: 0,
                            change: 0.0,
                            day_id: 0,
                            day_volume: 0.0,
                            day_turnover: 0.0,
                            raw_flags: 0,
                            direction: 0,
                            is_eth: 0,
                            scope: 0,
                        });
                        Ok(dxfeed::Event { sym: symbol, data })
                    }
                    MarketEvent::Greeks(greeks) => {
                        // Convert Greeks to dxfeed format.
                        // DXLink's GreeksEvent carries no price/time, so those stay 0.
                        let symbol = greeks.event_symbol;
                        let data = dxfeed::EventData::Greeks(dxfeed::DxfGreeksT {
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
                        });
                        Ok(dxfeed::Event { sym: symbol, data })
                    }
                }
            }
            None => {
                // Fallback to previous implementation
                self.event_receiver.recv_async().await
            }
        }
    }
}

impl Clone for QuoteSubscription {
    fn clone(&self) -> Self {
        // Create a new channel for DXLink events
        let (tx, rx) = mpsc::channel(100);

        // Register this new channel with the streamer
        if let Some(cmd_tx) = &self.streamer.commands {
            let cmd_tx_clone = cmd_tx.clone();
            let sub_id = self.id.0;

            tokio::spawn(async move {
                if let Err(e) = cmd_tx_clone
                    .send(DXLinkCommand::AddEventSender(sub_id as u32, tx))
                    .await
                {
                    error!("Failed to register cloned event sender: {}", e);
                }
            });
        }

        Self {
            id: self.id,
            streamer: self.streamer.clone(),
            event_types: self.event_types,
            event_receiver: self.event_receiver.clone(), // This requires flume::Receiver to implement Clone
            dxlink_receiver: rx,
            symbols: self.symbols.clone(),
        }
    }
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

/// The DXLink subscription requests for `symbols` under `event_types`.
///
/// One request per symbol per event type the flags ask for. Shared by
/// subscribing, unsubscribing and the reconnect replay, so a symbol is
/// restored under exactly the event types it was subscribed with.
fn feed_subscriptions(symbols: &[Symbol], event_types: i32) -> Vec<FeedSubscription> {
    const FLAGS: [(i32, &str); 3] = [
        (dxfeed::DXF_ET_QUOTE, "Quote"),
        (dxfeed::DXF_ET_TRADE, "Trade"),
        (dxfeed::DXF_ET_GREEKS, "Greeks"),
    ];

    symbols
        .iter()
        .flat_map(|symbol| {
            FLAGS
                .iter()
                .filter(move |(flag, _)| event_types & flag != 0)
                .map(move |(_, name)| FeedSubscription {
                    event_type: (*name).to_string(),
                    symbol: symbol.0.clone(),
                    from_time: None,
                    source: None,
                })
        })
        .collect()
}

/// Recovers a poisoned lock rather than panicking.
///
/// The value behind it is a set of symbol strings. A thread panicking while
/// holding the lock cannot leave that set in a state the next reader cannot
/// understand, so poisoning here carries no information worth aborting a
/// caller's process over.
fn symbols_of(set: &Mutex<BTreeSet<Symbol>>) -> std::sync::MutexGuard<'_, BTreeSet<Symbol>> {
    set.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

enum DXLinkCommand {
    // No channel id: the caller's copy is a snapshot from connect time, and a
    // reconnect opens a new channel. The supervisor addresses the live one.
    Subscribe(
        Vec<FeedSubscription>,
        u32,
        Option<oneshot::Sender<TastyResult<()>>>,
    ),
    Unsubscribe(
        Vec<FeedSubscription>,
        u32,
        Option<oneshot::Sender<TastyResult<()>>>,
    ),
    AddEventSender(u32, mpsc::Sender<MarketEvent>),
    RemoveEventSender(u32),
}

// Live routing registry shared between the command loop and the event
// forwarding task, so senders registered at any time are always visible.
#[derive(Default)]
struct EventRouting {
    senders: HashMap<u32, Vec<mpsc::Sender<MarketEvent>>>,
    symbol_subs: HashMap<String, HashSet<u32>>,
}

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
    event_types: i32,
    symbols: Arc<Mutex<BTreeSet<Symbol>>>,
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

        tokio::spawn(supervise(
            tasty.clone(),
            policy,
            connection,
            command_rx,
            shutdown_rx,
            routing,
            registry.clone(),
            state.clone(),
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

    /// Create a subscription to market data. See `dxfeed::DXF_ET_*` for possible event types.
    pub fn create_sub(&mut self, flags: i32) -> Box<QuoteSubscription> {
        let id = SubscriptionId(self.next_sub_id);
        self.next_sub_id += 1;

        // Set up channels for events
        let (dxlink_tx, dxlink_rx) = mpsc::channel(100);
        let (_event_sender, event_receiver) = flume::unbounded();

        // Register the event sender. There is no stream to ask for any more:
        // the supervisor forwards from the receiver `connect` returns, from
        // the moment each connection is established.
        if let Some(client_tx) = &self.dxlink_command_tx {
            let client_tx_clone = client_tx.clone();
            let sub_id = id.0 as u32;

            tokio::spawn(async move {
                if let Err(e) = client_tx_clone
                    .send(DXLinkCommand::AddEventSender(sub_id, dxlink_tx))
                    .await
                {
                    error!("Failed to register event sender: {}", e);
                }
            });
        }

        // Create subscription
        let symbols = Arc::new(Mutex::new(BTreeSet::new()));

        // The supervisor replays from this. Registering the shared set rather
        // than a copy is what makes the replay send exactly what the venue
        // confirmed, including symbols added long after this call.
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                id.0 as u32,
                SubscriptionRecord {
                    event_types: flags,
                    symbols: symbols.clone(),
                },
            );

        let subscription = QuoteSubscription {
            id,
            streamer: self.handle(),
            event_types: flags,
            event_receiver,
            dxlink_receiver: dxlink_rx,
            symbols,
        };

        // Store subscription in map and return a boxed clone
        let sub_clone = subscription.clone();
        self.subscription_map.insert(id, subscription);

        Box::new(sub_clone)
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
            let symbols: Vec<Symbol> = symbols_of(&subscription.symbols).iter().cloned().collect();

            let unsubscribe_requests = feed_subscriptions(&symbols, subscription.event_types);

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
                symbols_of(&subscription.symbols).clear();
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

    /// Does nothing.
    ///
    /// Superseded by [`QuoteSubscription::add_symbols`], which subscribes
    /// against a specific subscription and reports whether the venue accepted
    /// it. Kept so existing callers still compile; it logs a warning and
    /// returns.
    pub fn subscribe(&self, _symbol: &[&str]) {
        // This method is deprecated - use QuoteSubscription::add_symbols() instead
        warn!(
            "QuoteStreamer::subscribe() is deprecated. Use QuoteSubscription::add_symbols() instead."
        );
    }

    /// Always returns `Err(RecvError::Disconnected)`.
    ///
    /// Events belong to a subscription, so use
    /// [`QuoteSubscription::get_event`]. Kept so existing callers still
    /// compile.
    pub async fn get_event(&self) -> std::result::Result<dxfeed::Event, flume::RecvError> {
        // This method is deprecated - use QuoteSubscription::get_event() instead
        // Return an error indicating this method should not be used
        Err(flume::RecvError::Disconnected)
    }
}

/// One live DXLink connection and everything the supervisor needs from it.
struct LiveConnection {
    client: DXLinkClient,
    channel_id: u32,
    /// The receiver `DXLinkClient::connect` hands back.
    ///
    /// It is the *only* path market events take out of dxlink: the client's
    /// event sender is created inside `connect`, so a later `event_stream()`
    /// call is refused with "Event stream already created". Dropping this
    /// receiver therefore does not merely lose a convenience — it disconnects
    /// the feed from its consumer permanently, which is what used to happen
    /// here.
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

    client
        .setup_feed(
            channel_id,
            &[EventType::Quote, EventType::Trade, EventType::Greeks],
        )
        .await
        .map_err(TastyTradeError::from)?;

    Ok(LiveConnection {
        client,
        channel_id,
        events,
    })
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
async fn forward_events(
    mut events: mpsc::Receiver<MarketEvent>,
    routing: Arc<RwLock<EventRouting>>,
    saw_event: Arc<AtomicBool>,
) {
    while let Some(event) = events.recv().await {
        saw_event.store(true, Ordering::Relaxed);

        let symbol = match &event {
            MarketEvent::Quote(quote) => &quote.event_symbol,
            MarketEvent::Trade(trade) => &trade.event_symbol,
            MarketEvent::Greeks(greeks) => &greeks.event_symbol,
        };

        let routing = routing.read().await;
        let Some(sub_ids) = routing.symbol_subs.get(symbol) else {
            debug!("No subscription registered for symbol {}", symbol);
            continue;
        };
        for sub_id in sub_ids {
            if let Some(sender_list) = routing.senders.get(sub_id) {
                for sender in sender_list {
                    // A consumer that is not keeping up loses events rather
                    // than stalling everyone else's.
                    let _ = sender.try_send(event.clone());
                }
            }
        }
    }
}

/// Re-subscribes every symbol the subscriptions still hold.
///
/// Returns whether all of them landed. A partial replay is reported as a
/// failure, because a subscription silently missing half its symbols is worse
/// than one more reconnect.
async fn replay(client: &mut DXLinkClient, channel_id: u32, registry: &Registry) -> bool {
    let pending = pending_replay(registry);

    if pending.is_empty() {
        return true;
    }

    debug!(
        "Restoring {} subscription(s) after a reconnect",
        pending.len()
    );
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
fn pending_replay(registry: &Registry) -> Vec<(u32, Vec<FeedSubscription>)> {
    let registry = registry.lock().unwrap_or_else(|p| p.into_inner());
    registry
        .iter()
        .map(|(sub_id, record)| {
            let symbols: Vec<Symbol> = symbols_of(&record.symbols).iter().cloned().collect();
            (*sub_id, feed_subscriptions(&symbols, record.event_types))
        })
        .filter(|(_, requests)| !requests.is_empty())
        .collect()
}

/// Records that `sub_id` wants events for these symbols.
///
/// Called before the subscribe is written, so no event can arrive for a symbol
/// that has no route yet.
async fn record_routes(
    routing: &Arc<RwLock<EventRouting>>,
    sub_id: u32,
    subscriptions: &[FeedSubscription],
) {
    let mut routing = routing.write().await;
    for sub in subscriptions {
        routing
            .symbol_subs
            .entry(sub.symbol.clone())
            .or_default()
            .insert(sub_id);
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
    for sub in subscriptions {
        if let Some(subs) = routing.symbol_subs.get_mut(&sub.symbol) {
            subs.remove(&sub_id);
            if subs.is_empty() {
                routing.symbol_subs.remove(&sub.symbol);
            }
        }
    }
}

/// Runs one connection until it is lost or the owner goes away.
async fn run_connection(
    client: &mut DXLinkClient,
    channel_id: u32,
    commands: &mut mpsc::Receiver<DXLinkCommand>,
    shutdown: &mut oneshot::Receiver<()>,
    routing: &Arc<RwLock<EventRouting>>,
) -> Ended {
    loop {
        let cmd = tokio::select! {
            biased;
            _ = &mut *shutdown => {
                debug!("Quote streamer owner dropped, disconnecting");
                return Ended::Owner;
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
            DXLinkCommand::Subscribe(subscriptions, sub_id, ack) => {
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
                routing.symbol_subs.retain(|_, subs| {
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
        let forwarder = tokio::spawn(forward_events(events, routing.clone(), saw_event.clone()));

        let restored = replay(&mut client, channel_id, &registry).await;
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
                &routing,
            )
            .await
        } else {
            warn!("Could not restore every subscription; reconnecting");
            Ended::ConnectionLost
        };

        forwarder.abort();
        // dxlink's message pump never exits on its own — on a read error it
        // logs, sleeps 100ms and loops — so a client that is not disconnected
        // leaves a task spinning for the life of the process, once per
        // reconnect.
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

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        sub.add_symbols(&[Symbol::from("AAPL"), Symbol::from("MSFT")])
            .await
            .expect("subscribing succeeds");

        // The streamer's own copy sees them, which is what close_sub reads.
        {
            let stored = symbols_of(
                &streamer
                    .subscription_map
                    .get(&sub.id)
                    .expect("the streamer kept a copy")
                    .symbols,
            );
            assert_eq!(stored.len(), 2, "the streamer's copy must see the symbols");
            assert!(stored.contains(&Symbol::from("AAPL")));
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

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        let error = sub
            .add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect_err("a refused subscription is not a success");

        // The venue's own answer reaches the caller rather than being
        // flattened into a generic failure.
        assert!(format!("{error}").contains("venue said no"), "{error}");
        assert!(
            symbols_of(&sub.symbols).is_empty(),
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

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
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

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        drop(rx); // the command loop is gone

        sub.add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect_err("a closed streamer cannot subscribe");

        assert!(
            symbols_of(&sub.symbols).is_empty(),
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
                    DXLinkCommand::Subscribe(requests, _, ack) => {
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
        let requests = feed_subscriptions(
            &[Symbol::from("AAPL"), Symbol::from("MSFT")],
            dxfeed::DXF_ET_QUOTE | dxfeed::DXF_ET_GREEKS,
        );

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
                event_types: dxfeed::DXF_ET_QUOTE,
                symbols: Arc::new(Mutex::new(BTreeSet::new())),
            },
        );

        assert!(
            pending_replay(&registry).is_empty(),
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

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        sub.add_symbols(&[Symbol::from("AAPL")])
            .await
            .expect("subscribing succeeds");

        let pending = pending_replay(&streamer.registry);
        assert_eq!(pending.len(), 1, "a live subscription is restored");
        assert_eq!(pending[0].1[0].symbol, "AAPL");

        streamer.close_sub(sub.id).await.expect("closing succeeds");

        assert!(
            pending_replay(&streamer.registry).is_empty(),
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
            routing.senders.insert(1, vec![sub_tx]);
            routing
                .symbol_subs
                .insert("AAPL".to_string(), HashSet::from([1]));
        }

        let (events_tx, events_rx) = mpsc::channel::<MarketEvent>(4);
        let saw_event = Arc::new(AtomicBool::new(false));
        let forwarder = tokio::spawn(forward_events(
            events_rx,
            routing.clone(),
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
        let wanted = feed_subscriptions(&[Symbol::from("AAPL")], dxfeed::DXF_ET_QUOTE);

        // Two subscriptions ask for the same symbol; one of them is refused.
        record_routes(&routing, 3, &wanted).await;
        record_routes(&routing, 4, &wanted).await;
        forget_routes(&routing, 3, &wanted).await;

        let routes = routing.read().await;
        let subs = routes
            .symbol_subs
            .get("AAPL")
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
        let wanted = feed_subscriptions(&[Symbol::from("AAPL")], dxfeed::DXF_ET_QUOTE);

        record_routes(&routing, 3, &wanted).await;
        forget_routes(&routing, 3, &wanted).await;

        assert!(
            routing.read().await.symbol_subs.is_empty(),
            "an empty set must not be left behind as a route"
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
