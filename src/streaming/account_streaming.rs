use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};

use crate::TastyTradeError;
use crate::accounts::AccountNumber;
use crate::streaming::reconnect::{BackoffPolicy, ConnectionState};
use crate::types::balance::Balance;
use crate::{BriefPosition, LiveOrderRecord, TastyResult, TastyTrade, accounts::Account};
use futures_util::{SinkExt, StreamExt};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, warn};

/**
Represents the different types of subscription requests.  Used for managing real-time data streams.
*/
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubRequestAction {
    /// Represents a heartbeat message.  Used to maintain an active connection.
    Heartbeat,
    /// Represents a connection request.  Initiates a new data stream.
    Connect,
    /// Represents a subscription request for public watchlists.
    PublicWatchlistsSubscribe,
    /// Represents a subscription request for quote alerts.
    QuoteAlertsSubscribe,
    /// Represents a subscription request for user messages.
    UserMessageSubscribe,
}

impl std::fmt::Display for SubRequestAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubRequestAction::Heartbeat => write!(f, "heartbeat"),
            SubRequestAction::Connect => write!(f, "connect"),
            SubRequestAction::PublicWatchlistsSubscribe => write!(f, "public-watchlists-subscribe"),
            SubRequestAction::QuoteAlertsSubscribe => write!(f, "quote-alerts-subscribe"),
            SubRequestAction::UserMessageSubscribe => write!(f, "user-message-subscribe"),
        }
    }
}

/// Represents a subscription request.
///
/// This struct is used to send subscription requests to the server.
/// The `value` field is optional and its type depends on the `action` field.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SubRequest<T: Serialize> {
    /// Authentication token.
    auth_token: String,
    /// Action to be performed.
    action: SubRequestAction,
    /// Value associated with the action.  This field is optional.
    value: Option<T>,
}

/// Represents an action to be performed by a handler.
///
/// This struct encapsulates both the type of action to be executed and an optional
/// value associated with that action.  The value is dynamically typed and serializable,
/// allowing for flexibility in the data passed along with the action.
///
pub struct HandlerAction {
    /// The specific action to be performed.
    action: SubRequestAction,

    /// An optional value associated with the action.  This value, if present,
    /// must implement the `erased_serde::Serialize`, `Send`, and `Sync` traits.
    value: Option<Box<dyn erased_serde::Serialize + Send + Sync>>,

    /// Where the writer reports what actually happened.
    ///
    /// Reaching the in-process queue is not the same as reaching the venue.
    /// Serialisation and the websocket write both happen later and both can
    /// fail, so the outcome travels back rather than the caller being told
    /// "sent" while the work is still ahead of it.
    ack: Option<oneshot::Sender<TastyResult<()>>>,
}

/// Represents a message related to an account.
///
/// This enum uses the `serde` library's tagged enum representation.  The `type` field
/// in the JSON will determine which variant is used.  The `data` field will contain
/// the associated data for that variant.
///
/// # Examples
///
/// ```json
/// {"type": "order", "data": { ... order data ... }}
/// {"type": "account_balance", "data": { ... balance data ... }}
/// {"type": "current_position", "data": { ... position data ... }}
/// {"type": "order_chain", "data": null}
/// {"type": "external_transaction", "data": null}
/// ```
#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum AccountMessage {
    /// Represents a live order record.  Contains a `LiveOrderRecord` struct.
    Order(LiveOrderRecord),
    /// Represents the account balance. Contains a `Balance` struct.
    AccountBalance(Box<Balance>),
    /// Represents the current position. Contains a `BriefPosition` struct.
    CurrentPosition(Box<BriefPosition>),
    /// Represents an order chain.  Currently has no associated data.
    OrderChain,
    /// Represents an external transaction.  Currently has no associated data.
    ExternalTransaction,
}

/// Represents a status message received from the API.
///
/// This struct is used to deserialize status messages, which provide information
/// about the status of a request, the action taken, and the WebSocket session ID.
///
/// # Example
///
/// ```json
/// {
///     "status": "success",
///     "action": "subscribe",
///     "web-socket-session-id": "a1b2c3d4-e5f6-7890-1234-567890abcdef",
///     "request-id": 12345
/// }
/// ```
#[derive(Deserialize, DebugPretty, DisplaySimple, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StatusMessage {
    /// The status of the request (e.g., "success", "error").
    pub status: String,
    /// The action performed (e.g., "subscribe", "unsubscribe").
    pub action: String,
    /// The ID of the WebSocket session.
    pub web_socket_session_id: String,
    /// The unique identifier for the request.
    pub request_id: u64,
}

/// Represents an error message received from the API.
///
/// This struct is deserialized from a JSON response and provides details about the error.
/// All fields are in kebab-case to match the API's naming convention.
#[derive(Deserialize, DebugPretty, DisplaySimple, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ErrorMessage {
    /// The status of the error.
    pub status: String,
    /// The action that caused the error.
    pub action: String,
    /// The ID of the WebSocket session where the error occurred.
    pub web_socket_session_id: String,
    /// A human-readable description of the error.
    pub message: String,
}

/// Represents the different types of events that can be received from the account streaming API.
///
/// This enum uses `serde`'s untagged enum representation.  This means the
/// deserialization will try each variant in order until one matches.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum AccountEvent {
    /// Represents an error message received from the API.
    ErrorMessage(ErrorMessage),
    /// Represents a status message received from the API.
    StatusMessage(StatusMessage),
    /// Represents an account-related message received from the API.  This variant
    /// is boxed to reduce the size of the `AccountEvent` enum.
    AccountMessage(Box<AccountMessage>),
}

/// Which transport an [`AccountStreamer`] is using.
///
/// One variant today. It exists so the choice is visible in the type rather
/// than implied by which fields happen to be `Some`, and so adding a transport
/// later is an added variant rather than a redesign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountTransport {
    /// The tastytrade account websocket, subscribed to with `SubRequest`
    /// messages and kept alive with a heartbeat.
    Websocket,
}

/// Streams account events: balances, orders and positions.
///
/// Exactly one transport is connected. An earlier version opened a DXLink
/// client *and* this websocket, subscribed on both, and forwarded events from
/// only one of them, so a consumer paid for two connections and received one
/// stream. #54 tracks what DXLink would need before it can be offered here.
#[derive(Debug)]
pub struct AccountStreamer {
    /// Receiver for account events.
    pub event_receiver: flume::Receiver<AccountEvent>,
    /// Sender for actions to be handled.
    pub action_sender: flume::Sender<HandlerAction>,
    /// The transport this streamer connected over.
    transport: AccountTransport,
    /// Where the connection stands, shared with the supervisor task.
    state: Arc<RwLock<ConnectionState>>,
    /// Ends the supervisor when this streamer is dropped.
    cancel: Option<oneshot::Sender<()>>,
    /// Accounts to resubscribe after a reconnect.
    ///
    /// A reconnect that silently forgets what it was watching is worse than
    /// one that fails, because the caller keeps waiting for events that will
    /// never come.
    subscribed: Arc<Mutex<BTreeSet<AccountNumber>>>,
}

impl AccountStreamer {
    /// Connects to the tastytrade account websocket.
    ///
    /// One connection, one stream. The previous implementation also stood up a
    /// DXLink client, created an `ACCOUNT` feed channel and subscribed on it,
    /// but nothing forwarded DXLink events to the receiver, so every event a
    /// caller ever saw came from this websocket while they paid for both. It
    /// also returned an error when DXLink failed to connect, which meant the
    /// fallback its own comments described could not happen.
    ///
    /// Offering DXLink here needs event forwarding written and a session
    /// against certification confirming tastytrade actually emits account
    /// events on that channel. That is #54. Until then this is the transport,
    /// and saying so is more useful than a fallback that never ran.
    ///
    /// # Arguments
    ///
    /// * `tasty` - A reference to the `TastyTrade` client, containing
    ///   authentication and configuration details.
    pub async fn connect(tasty: &TastyTrade) -> TastyResult<AccountStreamer> {
        Self::connect_with_policy(tasty, BackoffPolicy::default()).await
    }

    /// Connects with an explicit reconnection policy.
    ///
    /// The first connection is established before returning, so a caller that
    /// cannot reach the venue at all learns immediately rather than through a
    /// stream that never produces anything. Every later drop is handled by a
    /// supervisor: it waits out the backoff, logs in again to get a fresh
    /// session token, reconnects, and resubscribes the accounts that were
    /// subscribed before.
    pub async fn connect_with_policy(
        tasty: &TastyTrade,
        policy: BackoffPolicy,
    ) -> TastyResult<AccountStreamer> {
        let (event_sender, event_receiver) = flume::unbounded();
        let (action_sender, action_receiver): (
            flume::Sender<HandlerAction>,
            flume::Receiver<HandlerAction>,
        ) = flume::unbounded();

        // Prove the venue is reachable before handing back a streamer.
        let session = connect_session(&tasty.config.websocket_url).await?;
        debug!("Account websocket connected");

        let state = Arc::new(RwLock::new(ConnectionState::Connected));
        let subscribed: Arc<Mutex<BTreeSet<AccountNumber>>> = Arc::new(Mutex::new(BTreeSet::new()));

        let supervisor_state = state.clone();
        let supervisor_subscribed = subscribed.clone();
        let supervisor_actions = action_sender.clone();
        // The supervisor holds its own action sender, so the receiver never
        // closes on its own and a quiet socket would keep the loop alive after
        // its owner is gone. Only the streamer holds this.
        let (cancel_tx, mut cancelled) = oneshot::channel::<()>();
        let config = tasty.config.clone();
        let mut token = tasty.session_token.clone();
        let mut session = Some(session);

        tokio::spawn(async move {
            let mut attempt = 0u32;

            loop {
                let live = match session.take() {
                    Some(live) => live,
                    None => match connect_session(&config.websocket_url).await {
                        Ok(live) => live,
                        Err(e) => {
                            if !policy.should_retry(&e) {
                                terminal(&supervisor_state, format!("reconnect refused: {e}"))
                                    .await;
                                return;
                            }
                            match schedule(&policy, &mut attempt, &supervisor_state, &mut cancelled)
                                .await
                            {
                                true => continue,
                                false => return,
                            }
                        }
                    },
                };

                *supervisor_state.write().await = ConnectionState::Connected;

                let worked = run_session(
                    live,
                    &token,
                    &event_sender,
                    &action_receiver,
                    &mut cancelled,
                )
                .await;

                // Reset only for a session the venue actually accepted a write
                // on. Resetting on a successful handshake let a venue that
                // takes the socket and rejects the session loop forever at
                // attempt one.
                if worked {
                    attempt = 0;
                }

                if cancelled.try_recv().is_ok() || event_sender.is_disconnected() {
                    debug!("Account streamer dropped, ending the supervisor");
                    return;
                }

                if !schedule(&policy, &mut attempt, &supervisor_state, &mut cancelled).await {
                    return;
                }

                // The session token may be why the socket dropped, so take a
                // fresh one rather than presenting the same one again.
                match TastyTrade::login(&config).await {
                    Ok(client) => token = client.session_token.clone(),
                    Err(e) => {
                        if !policy.should_retry(&e) {
                            terminal(
                                &supervisor_state,
                                "re-authentication was refused; the credentials no longer work"
                                    .to_string(),
                            )
                            .await;
                            return;
                        }
                        // Keep the old token and try the socket anyway: the
                        // login endpoint being briefly unavailable does not
                        // mean the session is invalid.
                        warn!("Could not re-authenticate before reconnecting: {e}");
                    }
                }

                // Connected is claimed only once what was being watched is
                // watched again. Reporting it before restoration leaves a
                // caller believing they are receiving events they are not.
                if !resubscribe(&supervisor_actions, &supervisor_subscribed).await {
                    warn!("Could not restore every subscription; reconnecting again");
                    if !schedule(&policy, &mut attempt, &supervisor_state, &mut cancelled).await {
                        return;
                    }
                }
            }
        });

        Ok(Self {
            event_receiver,
            action_sender,
            transport: AccountTransport::Websocket,
            cancel: Some(cancel_tx),
            state,
            subscribed,
        })
    }

    /// Where the connection currently stands.
    ///
    /// Carries counts and durations only, never a token or an account, so it
    /// is safe to log or surface to a user.
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Which transport this streamer connected over.
    ///
    /// Observable without exposing tokens or account identifiers, so a caller
    /// can log or report it.
    pub fn transport(&self) -> AccountTransport {
        self.transport
    }

    /// Subscribes to updates for `account`.
    ///
    /// One subscription over one transport. The previous version also sent a
    /// DXLink subscribe for the same account, on a channel whose events never
    /// reached the caller.
    ///
    /// # Arguments
    ///
    /// * `account` - A reference to the `Account` object to subscribe to.
    pub async fn subscribe_to_account<'a>(&self, account: &'a Account<'a>) -> TastyResult<()> {
        let number = account.inner.account.account_number.clone();

        self.send(SubRequestAction::Connect, Some(vec![number.clone()]))
            .await?;

        // Recorded only after the venue accepted it, and it is what a
        // reconnect replays. A reconnect that silently forgets what it was
        // watching is worse than one that fails, because the caller keeps
        // waiting for events that will never come.
        subscribed_of(&self.subscribed).insert(number);

        Ok(())
    }

    /// Sends an action to the account streamer.
    ///
    /// This function sends a `HandlerAction` to the account streamer via the `action_sender` channel.
    /// The `HandlerAction` consists of a `SubRequestAction` and an optional value.  The value, if provided,
    /// must implement the `Serialize`, `Send`, `Sync`, and `'static` traits.  It is then boxed and erased
    /// using `erased_serde` to allow for dynamic dispatch.
    ///
    /// # Arguments
    ///
    /// * `action` - The `SubRequestAction` to send. This determines the type of action being requested.
    /// * `value` - An optional value associated with the action. This value is serialized and sent
    ///   along with the action.
    ///
    pub async fn send<T: Serialize + Send + Sync + 'static>(
        &self,
        action: SubRequestAction,
        value: Option<T>,
    ) -> TastyResult<()> {
        let (ack, answer) = oneshot::channel();

        self.action_sender
            .send_async(HandlerAction {
                action,
                value: value
                    .map(|inner| Box::new(inner) as Box<dyn erased_serde::Serialize + Send + Sync>),
                ack: Some(ack),
            })
            .await
            .map_err(|_| {
                TastyTradeError::Streaming(
                    "the account stream is closed; reconnect before sending again".to_string(),
                )
            })?;

        // Reaching the queue is not the same as reaching the venue, so wait for
        // the writer to say what happened rather than reporting success while
        // the work is still ahead of us.
        answer.await.map_err(|_| {
            TastyTradeError::Streaming(
                "the account stream closed before the action was sent".to_string(),
            )
        })?
    }

    /// Receives the next account event asynchronously.
    ///
    /// This method attempts to receive the next `AccountEvent` from the internal event receiver.
    /// It returns a `Result` indicating either the received `AccountEvent` or a `flume::RecvError`
    /// if the receiver is disconnected.
    ///
    pub async fn get_event(&self) -> std::result::Result<AccountEvent, flume::RecvError> {
        self.event_receiver.recv_async().await
    }
}

/// One live websocket, split for reading and writing.
type Session = (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
);

/// Opens one websocket session.
async fn connect_session(url: &str) -> TastyResult<Session> {
    let (stream, _response) = connect_async(url.to_string()).await?;
    Ok(stream.split())
}

/// Marks the connection as finished, with a reason a caller can act on.
async fn terminal(state: &Arc<RwLock<ConnectionState>>, reason: String) {
    warn!("Account stream gave up: {reason}");
    *state.write().await = ConnectionState::Disconnected { reason };
}

/// Waits out the backoff for the next attempt.
///
/// Returns false when the policy says to stop, having recorded why.
async fn schedule(
    policy: &BackoffPolicy,
    attempt: &mut u32,
    state: &Arc<RwLock<ConnectionState>>,
    cancelled: &mut oneshot::Receiver<()>,
) -> bool {
    *attempt = attempt.saturating_add(1);

    // Jitter source. A clock read is enough entropy to stop a fleet of
    // clients synchronising on the same venue restart, and it costs no
    // dependency.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);

    let Some(delay) = policy.delay_for(*attempt, nanos) else {
        terminal(state, format!("gave up after {} attempts", *attempt - 1)).await;
        return false;
    };

    debug!("Account stream reconnecting, attempt {attempt} in {delay:?}");
    *state.write().await = ConnectionState::Reconnecting {
        attempt: *attempt,
        delay,
    };
    // Cancellable: a caller who drops the streamer should not wait out a
    // thirty-second backoff for a task nobody is listening to.
    tokio::select! {
        _ = &mut *cancelled => false,
        _ = tokio::time::sleep(delay) => true,
    }
}

/// Re-sends a Connect for every account that was subscribed before the drop.
/// Replays every account that was subscribed before the drop.
///
/// Returns whether all of them landed. Each replay is acknowledged: a session
/// that fails partway leaves the rest queued, and the next full replay would
/// then append duplicates on top of them.
async fn resubscribe(
    actions: &flume::Sender<HandlerAction>,
    subscribed: &Arc<Mutex<BTreeSet<AccountNumber>>>,
) -> bool {
    let accounts: Vec<AccountNumber> = {
        let guard = subscribed
            .lock()
            .expect("subscription set is never poisoned");
        guard.iter().cloned().collect()
    };

    for account in accounts {
        let (ack, answered) = oneshot::channel();
        let action = HandlerAction {
            action: SubRequestAction::Connect,
            value: Some(Box::new(vec![account]) as Box<dyn erased_serde::Serialize + Send + Sync>),
            ack: Some(ack),
        };

        if actions.send_async(action).await.is_err() {
            return false;
        }
        match answered.await {
            Ok(Ok(())) => {}
            _ => return false,
        }
    }

    true
}

/// Runs one session until either half ends.
/// Runs one session until either half ends.
///
/// Returns whether the venue ever accepted a write. A socket that connects and
/// is then dropped never gets that far, which is what stops a venue that
/// accepts connections and rejects sessions from looping forever at attempt
/// one.
async fn run_session(
    session: Session,
    token: &str,
    events: &flume::Sender<AccountEvent>,
    actions: &flume::Receiver<HandlerAction>,
    cancelled: &mut oneshot::Receiver<()>,
) -> bool {
    let (mut write, mut read) = session;

    // Owned by the session rather than by a task of its own, so it dies with
    // the socket it is keeping alive instead of ticking on against a
    // connection that is gone.
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.tick().await; // the first tick is immediate
    let mut wrote_successfully = false;

    loop {
        tokio::select! {
            _ = &mut *cancelled => {
                debug!("Account streamer dropped, ending the session");
                return wrote_successfully;
            }
            _ = heartbeat.tick() => {
                let message = SubRequest::<Box<dyn erased_serde::Serialize + Send + Sync>> {
                    auth_token: token.to_string(),
                    action: SubRequestAction::Heartbeat,
                    value: None,
                };
                let Ok(text) = serde_json::to_string(&message) else {
                    continue;
                };
                if write.send(Message::Text(text.into())).await.is_err() {
                    debug!("Account websocket heartbeat failed, ending the session");
                    return wrote_successfully;
                }
                // The venue accepted an authenticated write, so this is a
                // session that actually worked.
                wrote_successfully = true;
            }
            frame = read.next() => {
                let Some(message) = frame else {
                    debug!("Account websocket stream ended");
                    return wrote_successfully;
                };
                let frame = match message {
                    Ok(frame) => frame,
                    Err(e) => {
                        error!("Account websocket read failed, ending the session: {e}");
                        return wrote_successfully;
                    }
                };

                // Control frames are protocol noise, not account data.
                let data = match frame {
                    Message::Text(text) => text.as_bytes().to_vec(),
                    Message::Binary(bytes) => bytes.to_vec(),
                    Message::Close(_) => {
                        debug!("Account websocket closed by the venue");
                        return wrote_successfully;
                    }
                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                };

                let Some(event) = decode_account_frame(&data) else {
                    continue;
                };

                if events.send_async(event).await.is_err() {
                    debug!("Account event receiver dropped, ending the session");
                    return wrote_successfully;
                }
            }
            action = actions.recv_async() => {
                let Ok(action) = action else {
                    debug!("Account action sender dropped, ending the session");
                    return wrote_successfully;
                };
                let ack = action.ack;
                let message = SubRequest::<Box<dyn erased_serde::Serialize + Send + Sync>> {
                    auth_token: token.to_string(),
                    action: action.action,
                    value: action.value,
                };
                let text = match serde_json::to_string(&message) {
                    Ok(text) => text,
                    Err(e) => {
                        // A caller's own Serialize failed. Their action is
                        // lost, which they must be told, but it is not a
                        // reason to drop the connection.
                        error!("Dropping an account action that could not be serialized: {e}");
                        report(ack, Err(TastyTradeError::Streaming(
                            "the action could not be serialized".to_string(),
                        )));
                        continue;
                    }
                };

                match write.send(Message::Text(text.into())).await {
                    Ok(()) => {
                        wrote_successfully = true;
                        report(ack, Ok(()));
                    }
                    Err(e) => {
                        debug!("Account websocket write failed: {e}");
                        report(ack, Err(TastyTradeError::Streaming(
                            "the account stream closed before the action was sent".to_string(),
                        )));
                        return wrote_successfully;
                    }
                }
            }
        }
    }
}

impl Drop for AccountStreamer {
    /// Ends the supervisor.
    ///
    /// A oneshot send is synchronous, so this works outside a Tokio runtime,
    /// where spawning would panic. Without it the supervisor outlives its
    /// owner: it holds its own action sender, so the receiver never closes on
    /// its own, and a quiet socket keeps the heartbeat and the reconnect loop
    /// running for nobody.
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

/// Recovers a poisoned lock rather than panicking.
///
/// The value behind it is a set of account numbers. A thread panicking while
/// holding the lock cannot leave that set in a state the next reader cannot
/// understand, so poisoning carries nothing worth aborting a caller's process
/// over.
fn subscribed_of(
    set: &Arc<Mutex<BTreeSet<AccountNumber>>>,
) -> std::sync::MutexGuard<'_, BTreeSet<AccountNumber>> {
    set.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Sends the outcome back to whoever is waiting, if anyone still is.
///
/// A caller that stopped waiting is not an error: dropping the receiver is how
/// a fire-and-forget caller opts out.
fn report(ack: Option<oneshot::Sender<TastyResult<()>>>, outcome: TastyResult<()>) {
    if let Some(ack) = ack {
        let _ = ack.send(outcome);
    }
}

/// Decodes one account frame, reporting a failure without its contents.
///
/// Split out so the privacy rule is testable without a socket. `serde_json`'s
/// `Display` renders the rejected value on a type mismatch, so an account
/// number in a frame would land in the log through the error itself — the
/// same trap this crate closed on the REST path.
fn decode_account_frame(data: &[u8]) -> Option<AccountEvent> {
    match serde_json::from_slice::<AccountEvent>(data) {
        Ok(event) => Some(event),
        Err(e) => {
            warn!(
                "Skipping an unreadable account frame ({} bytes): {:?} error at line {}, column {}",
                data.len(),
                e.classify(),
                e.line(),
                e.column()
            );
            debug!("account frame decode error: {e}");
            None
        }
    }
}

impl TastyTrade {
    /// Creates a new `AccountStreamer`.
    ///
    /// Connects to the tastytrade account websocket, which is the one
    /// transport this streamer offers. See [`AccountStreamer::connect`] for
    /// why, and what a second one would need first.
    ///
    /// # Returns
    ///
    /// * `Ok(AccountStreamer)` - If the connection is successful, returns an
    ///   `AccountStreamer` instance, which can be used to receive account events.
    /// * `Err(TastyTradeError)` - If an error occurs during connection or setup.
    pub async fn create_account_streamer(&self) -> TastyResult<AccountStreamer> {
        AccountStreamer::connect(self).await
    }
}

#[cfg(test)]
mod frame_privacy_tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing::Level;

    /// A value that must never reach a log, distinctive enough that a
    /// substring search cannot match it by accident.
    const ACCOUNT_NUMBER: &str = "SENTINEL-5WX00042";

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Captured {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().expect("not poisoned in tests")).into_owned()
        }
    }

    impl io::Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("not poisoned in tests")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn decode_capturing(data: &[u8], level: Level) -> (Option<AccountEvent>, String) {
        let logs = Captured::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(level)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();

        let event = tracing::subscriber::with_default(subscriber, || decode_account_frame(data));
        (event, logs.text())
    }

    /// The trap this crate already closed once on the REST path: a type
    /// mismatch renders the rejected value inside the serde error, so logging
    /// the error is logging the account data.
    #[test]
    fn an_unreadable_frame_never_logs_its_contents_at_warn() {
        // `status` wants a string; a number there makes serde quote the
        // neighbouring context, and the frame carries an account number.
        let frame = format!(
            r#"{{"type":"Order","data":{{"account-number":"{ACCOUNT_NUMBER}","status":12345}}}}"#
        );

        let (event, logs) = decode_capturing(frame.as_bytes(), Level::WARN);

        assert!(event.is_none(), "the frame must not decode");
        assert!(
            !logs.contains(ACCOUNT_NUMBER),
            "the account number reached the logs:\n{logs}"
        );
        assert!(
            logs.contains("bytes)") && logs.contains("error at line"),
            "the failure must still be diagnosable:\n{logs}"
        );
    }

    #[test]
    fn the_detail_is_available_one_level_down() {
        let frame = br#"{ not json at all"#;
        let (event, logs) = decode_capturing(frame, Level::DEBUG);

        assert!(event.is_none());
        assert!(
            logs.contains("decode error"),
            "DEBUG keeps the full error:\n{logs}"
        );
    }
}
