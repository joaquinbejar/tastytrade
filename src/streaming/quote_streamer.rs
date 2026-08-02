// For quote_streamer.rs
use crate::TastyTrade;
use crate::types::dxfeed;
use crate::{AsSymbol, Symbol, TastyResult, TastyTradeError};
use dxlink::{DXLinkClient, EventType, FeedSubscription, MarketEvent};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, error, info, warn};

#[derive(DebugPretty, DisplaySimple, Serialize, PartialEq, Eq, Hash, Clone, Copy)]
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
    channel_id: Option<u32>,
}

pub struct QuoteSubscription {
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
    /// Add symbols to subscription. See the "Note on symbology" section in [`QuoteSubscription`]
    pub async fn add_symbols<S: AsSymbol>(&self, symbols: &[S]) -> TastyResult<()> {
        let requested: Vec<Symbol> = symbols.iter().map(|sym| sym.as_symbol()).collect();

        // Only symbols that are not already subscribed. Asking the venue twice
        // for the same symbol is at best wasted work and at worst a duplicate
        // stream.
        let symbols: Vec<Symbol> = {
            let known = self.symbols.lock().expect("symbol set is never poisoned");
            requested
                .into_iter()
                .filter(|sym| !known.contains(sym))
                .collect()
        };

        // Prepare subscription requests for DXLink
        let subscriptions = symbols
            .iter()
            .flat_map(|sym| {
                let mut requests = Vec::new();

                // Transform dxfeed flags to DXLink event types
                let event_flags = self.event_types;

                if (event_flags & dxfeed::DXF_ET_QUOTE) != 0 {
                    requests.push(FeedSubscription {
                        event_type: "Quote".to_string(),
                        symbol: sym.0.clone(),
                        from_time: None,
                        source: None,
                    });
                }

                if (event_flags & dxfeed::DXF_ET_TRADE) != 0 {
                    requests.push(FeedSubscription {
                        event_type: "Trade".to_string(),
                        symbol: sym.0.clone(),
                        from_time: None,
                        source: None,
                    });
                }

                if (event_flags & dxfeed::DXF_ET_GREEKS) != 0 {
                    requests.push(FeedSubscription {
                        event_type: "Greeks".to_string(),
                        symbol: sym.0.clone(),
                        from_time: None,
                        source: None,
                    });
                }

                requests
            })
            .collect::<Vec<FeedSubscription>>();

        if subscriptions.is_empty() {
            return Ok(());
        }

        // Awaited rather than spawned. A detached task meant this returned
        // success before the command was even accepted, so a caller could not
        // tell a subscription that worked from one that never left, and it
        // panicked outright when called without a Tokio runtime.
        let sub_id = self.id.0 as u32;
        let (Some(channel_id), Some(tx)) = (self.streamer.channel_id, &self.streamer.commands)
        else {
            return Err(TastyTradeError::Streaming(
                "the quote streamer has no open channel; reconnect before subscribing".to_string(),
            ));
        };

        tx.send(DXLinkCommand::Subscribe(channel_id, subscriptions, sub_id))
            .await
            .map_err(|_| {
                TastyTradeError::Streaming(
                    "the quote streamer is closed; reconnect before subscribing".to_string(),
                )
            })?;

        // Recorded only after the command is accepted. A symbol that never
        // left the process must not be unsubscribed later as though it had.
        self.symbols
            .lock()
            .expect("symbol set is never poisoned")
            .extend(symbols);

        Ok(())
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
enum DXLinkCommand {
    Subscribe(u32, Vec<FeedSubscription>, u32),
    Unsubscribe(u32, Vec<FeedSubscription>, u32),
    CreateEventStream,
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

pub struct QuoteStreamer {
    #[allow(dead_code)]
    dxlink_client: Option<DXLinkClient>,
    /// Signals the command loop to disconnect.
    ///
    /// Held only by the owner, never by a handle, so it is dropped exactly when
    /// the streamer is. A oneshot cannot be refused for lack of room the way a
    /// `try_send` into the bounded command queue could.
    shutdown: Option<oneshot::Sender<()>>,
    channel_id: Option<u32>,
    next_sub_id: usize,
    subscription_map: HashMap<SubscriptionId, QuoteSubscription>,
    dxlink_command_tx: Option<mpsc::Sender<DXLinkCommand>>,
}

impl QuoteStreamer {
    pub async fn connect(tasty: &TastyTrade) -> TastyResult<Self> {
        let tokens = tasty.quote_streamer_tokens().await?;
        debug!(
            "Obtained tokens for DXLink (token len={})",
            tokens.token.len()
        );

        // Create DXLink client
        let mut client = DXLinkClient::new(&tokens.streamer_url, &tokens.token);

        // Connect to server
        info!("Connecting to DXLink server: {}", tokens.streamer_url);
        if let Err(e) = client.connect().await {
            return Err(TastyTradeError::Streaming(format!(
                "Error connecting to DXLink: {}",
                e
            )));
        }

        // Create channel for market data
        let channel_id = match client.create_feed_channel("AUTO").await {
            Ok(id) => id,
            Err(e) => {
                return Err(TastyTradeError::Streaming(format!(
                    "Error creating DXLink channel: {}",
                    e
                )));
            }
        };
        info!("DXLink channel created: {}", channel_id);

        // Configure feed for different event types
        if let Err(e) = client
            .setup_feed(
                channel_id,
                &[EventType::Quote, EventType::Trade, EventType::Greeks],
            )
            .await
        {
            return Err(TastyTradeError::Streaming(format!(
                "Error configuring DXLink feed: {}",
                e
            )));
        }

        // Create command channel
        let (command_tx, mut command_rx) = mpsc::channel::<DXLinkCommand>(100);

        // Shutdown travels on its own channel. Routing it through the bounded
        // command queue meant a full queue could discard it, and the loop
        // cannot simply exit when the command sender dies either, because
        // every subscription handle holds a clone of that sender.
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Spawn task to handle DXLink commands
        tokio::spawn(async move {
            // Routing registry shared with the event forwarding task, so
            // subscriptions registered after stream creation still receive events
            let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
            let mut stream_created = false;

            loop {
                let cmd = tokio::select! {
                    biased;
                    _ = &mut shutdown_rx => {
                        debug!("Quote streamer owner dropped, disconnecting");
                        if let Err(e) = client.disconnect().await {
                            warn!("Error disconnecting from DXLink: {}", e);
                        }
                        break;
                    }
                    cmd = command_rx.recv() => match cmd {
                        Some(cmd) => cmd,
                        None => break,
                    },
                };

                match cmd {
                    DXLinkCommand::Subscribe(channel_id, subscriptions, sub_id) => {
                        // Record symbol routing before subscribing, so no event
                        // can arrive for a symbol that has no route yet
                        {
                            let mut routing = routing.write().await;
                            for sub in &subscriptions {
                                routing
                                    .symbol_subs
                                    .entry(sub.symbol.clone())
                                    .or_default()
                                    .insert(sub_id);
                            }
                        }
                        if let Err(e) = client.subscribe(channel_id, subscriptions).await {
                            error!("Error subscribing to symbols: {}", e);
                        }
                    }
                    DXLinkCommand::Unsubscribe(channel_id, subscriptions, sub_id) => {
                        {
                            let mut routing = routing.write().await;
                            for sub in &subscriptions {
                                if let Some(subs) = routing.symbol_subs.get_mut(&sub.symbol) {
                                    subs.remove(&sub_id);
                                    if subs.is_empty() {
                                        routing.symbol_subs.remove(&sub.symbol);
                                    }
                                }
                            }
                        }
                        if let Err(e) = client.unsubscribe(channel_id, subscriptions).await {
                            error!("Error unsubscribing from symbols: {}", e);
                        }
                    }
                    DXLinkCommand::CreateEventStream => {
                        if stream_created {
                            debug!("Event stream already created, ignoring request");
                            continue;
                        }
                        match client.event_stream() {
                            Ok(mut rx) => {
                                debug!("Successfully created event stream");
                                stream_created = true;
                                let routing = routing.clone();

                                tokio::spawn(async move {
                                    while let Some(event) = rx.recv().await {
                                        // Determine which symbol this event is for
                                        let symbol = match &event {
                                            MarketEvent::Quote(quote) => &quote.event_symbol,
                                            MarketEvent::Trade(trade) => &trade.event_symbol,
                                            MarketEvent::Greeks(greeks) => &greeks.event_symbol,
                                        };

                                        // Forward only to subscriptions registered for this symbol
                                        let routing = routing.read().await;
                                        let Some(sub_ids) = routing.symbol_subs.get(symbol) else {
                                            debug!(
                                                "No subscription registered for symbol {}",
                                                symbol
                                            );
                                            continue;
                                        };
                                        for sub_id in sub_ids {
                                            if let Some(sender_list) = routing.senders.get(sub_id) {
                                                for sender in sender_list {
                                                    // Try to send, but don't block if receiver is full
                                                    let _ = sender.try_send(event.clone());
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                error!("Failed to create event stream: {}", e);
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
            debug!("DXLink command handler terminated");
        });

        Ok(Self {
            dxlink_client: None,
            shutdown: Some(shutdown_tx), // We moved client into the command handler task
            channel_id: Some(channel_id),
            next_sub_id: 0,
            subscription_map: HashMap::new(),
            dxlink_command_tx: Some(command_tx),
        })
    }

    /// A handle a subscription can hold without owning the connection.
    fn handle(&self) -> StreamerHandle {
        StreamerHandle {
            commands: self.dxlink_command_tx.clone(),
            channel_id: self.channel_id,
        }
    }

    /// Create a subscription to market data. See `dxfeed::DXF_ET_*` for possible event types.
    pub fn create_sub(&mut self, flags: i32) -> Box<QuoteSubscription> {
        let id = SubscriptionId(self.next_sub_id);
        self.next_sub_id += 1;

        // Set up channels for events
        let (dxlink_tx, dxlink_rx) = mpsc::channel(100);
        let (_event_sender, event_receiver) = flume::unbounded();

        // Register event sender if we have a command channel
        if let Some(client_tx) = &self.dxlink_command_tx {
            let client_tx_clone = client_tx.clone();
            let sub_id = id.0 as u32;
            let needs_stream = self.subscription_map.is_empty() && self.channel_id.is_some();

            // Send both commands from a single task so the sender is always
            // registered before the event stream is created (the command loop
            // processes them in order)
            tokio::spawn(async move {
                if let Err(e) = client_tx_clone
                    .send(DXLinkCommand::AddEventSender(sub_id, dxlink_tx))
                    .await
                {
                    error!("Failed to register event sender: {}", e);
                    return;
                }

                if needs_stream {
                    match client_tx_clone.send(DXLinkCommand::CreateEventStream).await {
                        Ok(_) => debug!("Successfully requested event stream"),
                        Err(e) => error!("Failed to request event stream: {}", e),
                    }
                }
            });
        }

        // Create subscription
        let subscription = QuoteSubscription {
            id,
            streamer: self.handle(),
            event_types: flags,
            event_receiver,
            dxlink_receiver: dxlink_rx,
            symbols: Arc::new(Mutex::new(BTreeSet::new())),
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

    /// Close and remove subscription by id.
    /// Close and remove subscription by id.
    pub async fn close_sub(&mut self, id: SubscriptionId) -> TastyResult<()> {
        // Get symbols from subscription to close. This is the shared set, so
        // it holds what add_symbols actually subscribed rather than the empty
        // vector this used to read.
        if let Some(subscription) = self.subscription_map.get(&id) {
            let symbols: Vec<Symbol> = subscription
                .symbols
                .lock()
                .expect("symbol set is never poisoned")
                .iter()
                .cloned()
                .collect();

            // Prepare unsubscribe requests
            let unsubscribe_requests = symbols
                .iter()
                .flat_map(|sym| {
                    let mut requests = Vec::new();
                    let event_flags = subscription.event_types;

                    if (event_flags & dxfeed::DXF_ET_QUOTE) != 0 {
                        requests.push(FeedSubscription {
                            event_type: "Quote".to_string(),
                            symbol: sym.0.clone(),
                            from_time: None,
                            source: None,
                        });
                    }

                    if (event_flags & dxfeed::DXF_ET_TRADE) != 0 {
                        requests.push(FeedSubscription {
                            event_type: "Trade".to_string(),
                            symbol: sym.0.clone(),
                            from_time: None,
                            source: None,
                        });
                    }

                    if (event_flags & dxfeed::DXF_ET_GREEKS) != 0 {
                        requests.push(FeedSubscription {
                            event_type: "Greeks".to_string(),
                            symbol: sym.0.clone(),
                            from_time: None,
                            source: None,
                        });
                    }

                    requests
                })
                .collect::<Vec<FeedSubscription>>();

            // Awaited rather than spawned, so a caller learns whether the
            // venue was actually told. The order matters: stop routing events
            // to this subscription before telling the venue to stop sending
            // them, so nothing arrives for a route that is already gone.
            if let (Some(tx), Some(channel_id)) = (&self.dxlink_command_tx, self.channel_id) {
                let sub_id = id.0 as u32;

                let closed = |_| {
                    TastyTradeError::Streaming(
                        "the quote streamer is closed; the subscription is gone with it"
                            .to_string(),
                    )
                };

                tx.send(DXLinkCommand::RemoveEventSender(sub_id))
                    .await
                    .map_err(closed)?;

                if !unsubscribe_requests.is_empty() {
                    tx.send(DXLinkCommand::Unsubscribe(
                        channel_id,
                        unsubscribe_requests,
                        sub_id,
                    ))
                    .await
                    .map_err(closed)?;
                }
            }

            // The symbols are no longer subscribed, so the shared set must not
            // still claim they are.
            if let Some(subscription) = self.subscription_map.get(&id) {
                subscription
                    .symbols
                    .lock()
                    .expect("symbol set is never poisoned")
                    .clear();
            }
        }

        // Remove subscription from map
        self.subscription_map.remove(&id);

        Ok(())
    }

    pub fn subscribe(&self, _symbol: &[&str]) {
        // This method is deprecated - use QuoteSubscription::add_symbols() instead
        warn!(
            "QuoteStreamer::subscribe() is deprecated. Use QuoteSubscription::add_symbols() instead."
        );
    }

    pub async fn get_event(&self) -> std::result::Result<dxfeed::Event, flume::RecvError> {
        // This method is deprecated - use QuoteSubscription::get_event() instead
        // Return an error indicating this method should not be used
        Err(flume::RecvError::Disconnected)
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
        let (tx, mut rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        sub.add_symbols(&[Symbol::from("AAPL"), Symbol::from("MSFT")])
            .await
            .expect("subscribing succeeds");

        // The streamer's own copy sees them, which is what close_sub reads.
        let stored = streamer
            .subscription_map
            .get(&sub.id)
            .expect("the streamer kept a copy")
            .symbols
            .lock()
            .expect("not poisoned");
        assert_eq!(stored.len(), 2, "the streamer's copy must see the symbols");
        assert!(stored.contains(&Symbol::from("AAPL")));
        drop(stored);

        // And the subscribe command actually carried them.
        let mut sent = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            if let DXLinkCommand::Subscribe(_, requests, _) = cmd {
                sent.extend(requests.into_iter().map(|r| r.symbol));
            }
        }
        assert!(sent.contains(&"AAPL".to_string()));
        assert!(sent.contains(&"MSFT".to_string()));
    }

    /// A set, so asking twice subscribes once.
    #[tokio::test]
    async fn a_repeated_symbol_is_not_subscribed_twice() {
        let (tx, mut rx) = mpsc::channel::<DXLinkCommand>(8);
        let (shutdown_tx, _shutdown_rx) = oneshot::channel::<()>();
        let mut streamer = streamer_with(tx, shutdown_tx);

        let sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);
        sub.add_symbols(&[Symbol::from("AAPL")]).await.unwrap();
        sub.add_symbols(&[Symbol::from("AAPL")]).await.unwrap();

        let mut subscribes = 0;
        while let Ok(cmd) = rx.try_recv() {
            if matches!(cmd, DXLinkCommand::Subscribe(..)) {
                subscribes += 1;
            }
        }
        assert_eq!(subscribes, 1, "the second request had nothing new to say");
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
            sub.symbols.lock().expect("not poisoned").is_empty(),
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
            channel_id: Some(1),
        };
        let second = handle.clone();

        drop(handle);
        drop(second);

        // Nothing was sent, and the channel is still usable by its owner.
        assert!(
            rx.try_recv().is_err(),
            "dropping a handle must not send a command"
        );
        tx.send(DXLinkCommand::CreateEventStream)
            .await
            .expect("the owner's channel is still alive after handles are dropped");
        assert!(matches!(
            rx.recv().await,
            Some(DXLinkCommand::CreateEventStream)
        ));
    }

    /// `Drop` must not need a Tokio runtime. `try_send` is synchronous;
    /// `tokio::spawn` would panic here, which is what this asserts by simply
    /// not panicking.
    fn streamer_with(
        commands: mpsc::Sender<DXLinkCommand>,
        shutdown: oneshot::Sender<()>,
    ) -> QuoteStreamer {
        QuoteStreamer {
            dxlink_client: None,
            shutdown: Some(shutdown),
            channel_id: Some(1),
            next_sub_id: 0,
            subscription_map: HashMap::new(),
            dxlink_command_tx: Some(commands),
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

        tx.try_send(DXLinkCommand::CreateEventStream)
            .expect("the first command fits");
        assert!(
            tx.try_send(DXLinkCommand::CreateEventStream).is_err(),
            "the queue must actually be full for this test to mean anything"
        );

        drop(streamer_with(tx, shutdown_tx));

        assert!(
            shutdown_rx.try_recv().is_ok(),
            "a full command queue must not be able to swallow the shutdown"
        );
    }
}
