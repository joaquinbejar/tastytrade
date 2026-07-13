// For quote_streamer.rs
use crate::TastyTrade;
use crate::types::dxfeed;
use crate::{AsSymbol, Symbol, TastyResult, TastyTradeError};
use dxlink::{DXLinkClient, EventType, FeedSubscription, MarketEvent};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

#[derive(DebugPretty, DisplaySimple, Serialize, PartialEq, Eq, Hash, Clone, Copy)]
pub struct SubscriptionId(usize);

pub struct QuoteSubscription {
    pub id: SubscriptionId,
    streamer: Arc<Mutex<QuoteStreamer>>,
    event_types: i32, // Keep for compatibility with existing code
    event_receiver: flume::Receiver<dxfeed::Event>, // Keep for compatibility
    dxlink_receiver: mpsc::Receiver<MarketEvent>, // New DXLink event receiver
    symbols: Vec<Symbol>, // To track subscribed symbols
}

impl QuoteSubscription {
    /// Add symbols to subscription. See the "Note on symbology" section in [`QuoteSubscription`]
    pub fn add_symbols<S: AsSymbol>(&self, symbols: &[S]) {
        let symbols: Vec<Symbol> = symbols.iter().map(|sym| sym.as_symbol()).collect();

        // Update subscribed symbols internally
        let mut my_symbols = Vec::new();
        for sym in &symbols {
            my_symbols.push(sym.clone());
        }

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

        // Execute the subscription in a new async task
        let streamer_clone = self.streamer.clone();
        let subscriptions_clone = subscriptions.clone();
        let sub_id = self.id.0 as u32;

        tokio::spawn(async move {
            // Get the data we need from the mutex before awaiting
            let (channel_id, tx) = {
                if let Ok(streamer_guard) = streamer_clone.lock() {
                    // Extract what we need from the guard
                    let channel_id = streamer_guard.channel_id;
                    let tx = streamer_guard.dxlink_command_tx.clone();
                    (channel_id, tx)
                } else {
                    // If we can't lock the mutex, just return early
                    return;
                }
            }; // MutexGuard is dropped here

            // Now we're safe to await since we no longer hold the MutexGuard
            if let (Some(channel_id), Some(tx)) = (channel_id, tx) {
                // Send subscribe command through the channel
                if !subscriptions_clone.is_empty()
                    && let Err(e) = tx
                        .send(DXLinkCommand::Subscribe(
                            channel_id,
                            subscriptions_clone,
                            sub_id,
                        ))
                        .await
                {
                    error!("Failed to send subscription command: {}", e);
                }
            }
        });
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
        if let Ok(streamer) = self.streamer.lock()
            && let Some(cmd_tx) = &streamer.dxlink_command_tx
        {
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
    Disconnect,
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
    channel_id: Option<u32>,
    subscriptions: Arc<Mutex<HashMap<Symbol, Vec<String>>>>,
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

        // Spawn task to handle DXLink commands
        tokio::spawn(async move {
            // Routing registry shared with the event forwarding task, so
            // subscriptions registered after stream creation still receive events
            let routing: Arc<RwLock<EventRouting>> = Arc::new(RwLock::new(EventRouting::default()));
            let mut stream_created = false;

            while let Some(cmd) = command_rx.recv().await {
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
                    DXLinkCommand::Disconnect => {
                        if let Err(e) = client.disconnect().await {
                            warn!("Error disconnecting from DXLink: {}", e);
                        }
                        break; // Exit the loop after disconnecting
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
            dxlink_client: None, // We moved client into the command handler task
            channel_id: Some(channel_id),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_sub_id: 0,
            subscription_map: HashMap::new(),
            dxlink_command_tx: Some(command_tx),
        })
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
            streamer: Arc::new(Mutex::new(self.clone())), // Clone self
            event_types: flags,
            event_receiver,
            dxlink_receiver: dxlink_rx,
            symbols: Vec::new(),
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
    pub fn close_sub(&mut self, id: SubscriptionId) {
        // Get symbols from subscription to close
        if let Some(subscription) = self.subscription_map.get(&id) {
            let symbols = subscription.symbols.clone();

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

            // Execute unsubscribe via command channel
            if let (Some(tx), Some(channel_id)) = (&self.dxlink_command_tx, self.channel_id) {
                let tx_clone = tx.clone();
                let channel = channel_id;
                let requests = unsubscribe_requests.clone();
                let sub_id = id.0;

                tokio::spawn(async move {
                    // Unregister the event sender
                    if let Err(e) = tx_clone
                        .send(DXLinkCommand::RemoveEventSender(sub_id as u32))
                        .await
                    {
                        error!("Error unregistering event sender: {}", e);
                    }

                    // Unsubscribe from symbols
                    if !requests.is_empty()
                        && let Err(e) = tx_clone
                            .send(DXLinkCommand::Unsubscribe(channel, requests, sub_id as u32))
                            .await
                    {
                        error!("Error sending unsubscribe command: {}", e);
                    }
                });
            }
        }

        // Remove subscription from map
        self.subscription_map.remove(&id);
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

// Implement Clone for QuoteStreamer to support Arc<Mutex<Self>>
impl Clone for QuoteStreamer {
    fn clone(&self) -> Self {
        Self {
            dxlink_client: None, // Don't clone the client
            channel_id: self.channel_id,
            subscriptions: self.subscriptions.clone(),
            next_sub_id: self.next_sub_id,
            subscription_map: HashMap::new(), // Create a new empty map
            dxlink_command_tx: self.dxlink_command_tx.clone(),
        }
    }
}

impl Drop for QuoteStreamer {
    fn drop(&mut self) {
        // Clean up all subscriptions
        let subs_to_close: Vec<SubscriptionId> = self.subscription_map.keys().cloned().collect();
        for id in subs_to_close {
            self.close_sub(id);
        }

        // Signal disconnection
        if let Some(tx) = &self.dxlink_command_tx {
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                if let Err(e) = tx_clone.send(DXLinkCommand::Disconnect).await {
                    warn!("Error sending disconnect command: {}", e);
                }
            });
        }
    }
}
