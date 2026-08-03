use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};

use crate::TastyTradeError;
use crate::accounts::AccountNumber;
use crate::streaming::reconnect::{BackoffPolicy, ConnectionState};
use crate::types::balance::Balance;
use crate::types::quote_alert::QuoteAlert;
use crate::types::watchlist::Watchlist;
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
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct SubRequest<T: Serialize> {
    /// The OAuth2 access token, `Bearer `-prefixed.
    ///
    /// The venue documents this field as taking the same value as the
    /// `Authorization` header, prefix included — it is not a bare token. It is
    /// built by [`crate::oauth::AccessToken::bearer`] so the REST path and this
    /// one cannot drift.
    auth_token: String,
    /// Action to be performed.
    action: SubRequestAction,
    /// Value associated with the action.  This field is optional.
    value: Option<T>,
}

impl<T: Serialize> std::fmt::Debug for SubRequest<T> {
    /// Redacts the credential.
    ///
    /// The derived `Debug` printed the whole `Bearer …` value. Nothing logs a
    /// `SubRequest` today, which is exactly why it was easy to miss: the next
    /// person adding a trace to the writer would have leaked a live access
    /// token on the first line they wrote.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubRequest")
            .field("auth_token", &"***")
            .field("action", &self.action)
            .finish_non_exhaustive()
    }
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

/// The account payload a notification carries.
///
/// One variant per notification the four documented actions produce, plus one
/// for everything else. `Unsupported` is not a failure: it means the frame
/// arrived, its `type` is known to exist, and no captured frame has
/// established the schema — so the payload is kept rather than discarded, and
/// the caller can look at it.
#[derive(Debug)]
pub enum NotificationPayload {
    /// A full order object, published on every status change.
    ///
    /// The only place an executed price reaches a caller of this crate: fills
    /// live inside `legs` and no REST endpoint returns them.
    Order(Box<LiveOrderRecord>),
    /// A full account balance.
    AccountBalance(Box<Balance>),
    /// One position, as it now stands.
    CurrentPosition(Box<BriefPosition>),
    /// A quote alert the customer configured, and the venue fired.
    QuoteAlert(Box<QuoteAlert>),
    /// One of tastytrade's curated watchlists, as it now stands.
    PublicWatchlist(Box<Watchlist>),
    /// A notification whose `type` this crate recognises but does not model,
    /// or one whose payload did not decode.
    ///
    /// Both cases keep the payload. Discarding it was the old behaviour and it
    /// is the one thing a caller cannot recover from.
    Unsupported(RawPayload),
}

/// One notification from the account websocket.
///
/// The venue publishes a full object on every change, never a diff, so each of
/// these is a complete picture rather than something to merge into a previous
/// one.
#[derive(Debug)]
pub struct AccountNotification {
    /// The wire `type`, exactly as it arrived.
    ///
    /// Present even for the modelled variants: it is what a log line can name
    /// safely, and what tells two `Unsupported` payloads apart.
    pub kind: String,
    /// When the venue published it, in epoch milliseconds.
    pub timestamp: Option<i64>,
    /// What it is about.
    pub payload: NotificationPayload,
}

/// A frame this crate could not place.
///
/// Reached when the `type` is one nothing here recognises, or when the frame
/// is neither a notification nor a status message. The payload is kept: a
/// notification type added by the venue tomorrow arrives as one of these, and
/// a caller who knows what it is can read it.
#[derive(Debug)]
pub struct UnknownEvent {
    /// The `type` field, when the frame had one.
    pub kind: Option<String>,
    /// The `action` field, when the frame had one.
    pub action: Option<String>,
    /// The whole frame.
    pub payload: RawPayload,
}

/// JSON this crate did not model, kept without being made easy to leak.
///
/// Account frames carry account numbers, balances and venue prose. The value
/// belongs to the caller — it is their own account data — but it must not
/// travel by accident, so `Debug` and `Display` render a byte count and
/// nothing else, and there is no `Serialize`. Reading it takes
/// [`RawPayload::expose`], which is one grep away from an audit.
#[derive(Clone, PartialEq, Eq)]
pub struct RawPayload(String);

impl RawPayload {
    /// Wraps `json`.
    pub(crate) fn new(json: impl Into<String>) -> Self {
        Self(json.into())
    }

    /// The JSON text.
    ///
    /// Every call is a place account data can leave the process. There is one
    /// in this crate — none — and a caller adding one is choosing to.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// How many bytes it is. Safe to log; the contents are not.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there is nothing in it.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for RawPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RawPayload(<redacted, {} bytes>)", self.0.len())
    }
}

impl std::fmt::Display for RawPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<redacted, {} bytes>", self.0.len())
    }
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
///     "status": "ok",
///     "action": "connect",
///     "web-socket-session-id": "5b6e2799",
///     "value": ["5WT00000"],
///     "request-id": 2
/// }
/// ```
#[derive(Deserialize, DebugPretty, DisplaySimple, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StatusMessage {
    /// The status of the request. `ok` for an accepted action.
    pub status: String,
    /// The action performed, such as `connect` or `heartbeat`.
    pub action: String,
    /// The ID of the WebSocket session.
    ///
    /// `Option` because it is the venue's to send, and a frame that omits it
    /// is still an acknowledgement.
    #[serde(default)]
    pub web_socket_session_id: Option<String>,
    /// The identifier the request carried, echoed back.
    ///
    /// **`Option`, and that is the fix.** `request-id` is optional on the way
    /// out and the venue only echoes one it was given. This crate sends none,
    /// so it never came back — and a required `u64` here meant every status
    /// frame failed to deserialize, fell through the untagged enum, and was
    /// dropped with a warning. Acknowledgements were invisible.
    #[serde(default)]
    pub request_id: Option<u64>,
    /// What the action applied to, echoed back. `connect` returns the account
    /// numbers it subscribed.
    #[serde(default)]
    pub value: Option<Vec<AccountNumber>>,
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
    #[serde(default)]
    pub web_socket_session_id: Option<String>,
    /// A human-readable description of the error.
    ///
    /// Venue prose. It can name an account or a subscription, so it goes in
    /// front of a person and never into a log line.
    pub message: String,
}

/// Represents the different types of events that can be received from the account streaming API.
///
/// Decoded by looking at which fields the frame has, not by trying variants
/// until one sticks. The untagged version could not tell "a type this crate
/// does not model" from "a frame that is not JSON": both came out as a decode
/// failure and the event was dropped. A dropped event on this socket is a fill
/// the caller never hears about.
#[derive(Debug)]
pub enum AccountEvent {
    /// The venue refused an action.
    ErrorMessage(ErrorMessage),
    /// The venue acknowledged an action.
    StatusMessage(StatusMessage),
    /// An account notification.
    Notification(Box<AccountNotification>),
    /// A frame this crate could not place, kept rather than discarded.
    Unknown(UnknownEvent),
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
        // A clone of the client rather than a copied token. Access tokens last
        // about fifteen minutes and this connection is meant to last days, so
        // there is no such thing as "the" token for a session: every frame asks
        // the shared session for a live one, and the session refreshes when it
        // has to.
        let client = tasty.clone();
        let config = tasty.config.clone();
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
                    &client,
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

                // An expired access token may be why the socket dropped, so ask
                // the session for a live one before reconnecting rather than
                // presenting the same one again. There is no username and
                // password to fall back on: a refused refresh is the end.
                if let Err(e) = client.access_token().await {
                    if !policy.should_retry(&e) {
                        terminal(
                            &supervisor_state,
                            "the refresh token was refused; authorize again to obtain a new grant"
                                .to_string(),
                        )
                        .await;
                        return;
                    }
                    // A token endpoint that is briefly unavailable does not
                    // mean the grant is invalid, so this follows the same
                    // backoff as any other transient failure. The token in
                    // hand may still be good; if it is not, the next session
                    // fails and comes back through here.
                    warn!("Could not refresh the access token before reconnecting: {e}");
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

    /// Decodes one account frame without a socket.
    ///
    /// The same function the read loop uses. Public because "what does this
    /// crate do with a frame it does not model" is a question worth being able
    /// to answer without a connection, and because a caller replaying captured
    /// frames should get exactly the routing the live path gets.
    ///
    /// Returns `None` only when the bytes are not JSON. Everything else
    /// arrives as an [`AccountEvent`], with the payload kept even when nothing
    /// here can type it.
    pub fn decode_frame(data: &[u8]) -> Option<AccountEvent> {
        decode_account_frame(data)
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
    // Through `subscribed_of`, which recovers a poisoned lock. An `expect`
    // here would abort the caller's process over a set of account numbers a
    // panicking thread cannot have left in an unreadable state.
    let accounts: Vec<AccountNumber> = subscribed_of(subscribed).iter().cloned().collect();

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
    client: &TastyTrade,
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
                // Every frame carries a token, and a token lasts a quarter of
                // an hour, so the heartbeat is where a long-lived connection
                // discovers it needs a new one. Usually cached; a refusal here
                // ends the session and lets the supervisor's policy decide,
                // which is terminal for a rejected grant.
                let auth_token = match client.access_token().await {
                    Ok(token) => token.bearer(),
                    Err(e) => {
                        warn!("Ending the account session: no usable access token ({e})");
                        return wrote_successfully;
                    }
                };
                let message = SubRequest::<Box<dyn erased_serde::Serialize + Send + Sync>> {
                    auth_token,
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
                let auth_token = match client.access_token().await {
                    Ok(token) => token.bearer(),
                    Err(e) => {
                        // The caller is waiting on this one, so it gets the
                        // answer rather than a silent drop.
                        report(ack, Err(TastyTradeError::Auth(format!(
                            "the account stream has no usable access token: {e}"
                        ))));
                        return wrote_successfully;
                    }
                };
                let message = SubRequest::<Box<dyn erased_serde::Serialize + Send + Sync>> {
                    auth_token,
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

/// Notification types this crate recognises but does not model.
///
/// They are real — the venue publishes them, and other clients decode them —
/// but no captured frame here establishes their schema, and guessing one from
/// a field list produces a type that fails to decode the day it is wrong.
/// Naming them separately from the genuinely unknown is what lets a caller
/// tell "we have not typed this yet" from "the venue added something".
const OBSERVED_BUT_UNTYPED: [&str; 6] = [
    // Present in this crate's subscription actions but not in the venue's
    // current documentation.
    "UserMessage",
    // Legacy or observed variants. The old code had arms for the first two
    // that carried no data at all, so the payload was discarded outright.
    "OrderChain",
    "ExternalTransaction",
    "ComplexOrder",
    "TradingStatus",
    "UnderlyingYearGainSummary",
];

/// Decodes one account frame, reporting a failure without its contents.
///
/// Split out so the privacy rule is testable without a socket. `serde_json`'s
/// `Display` renders the rejected value on a type mismatch, so an account
/// number in a frame would land in the log through the error itself — the
/// same trap this crate closed on the REST path.
///
/// Returns `None` only when the bytes are not JSON at all. Everything that
/// *is* JSON reaches the caller: a notification if the `type` is one this
/// crate models, an acknowledgement if it looks like one, and an
/// [`AccountEvent::Unknown`] carrying the frame otherwise. The old
/// implementation asked serde to try three variants and dropped the frame when
/// none matched, which silently swallowed every status message — none of them
/// echo the `request-id` this crate never sends — along with any notification
/// type it did not model.
fn decode_account_frame(data: &[u8]) -> Option<AccountEvent> {
    let frame = match serde_json::from_slice::<serde_json::Value>(data) {
        Ok(frame) => frame,
        Err(e) => {
            // Classification, position and size. Never the error's own
            // rendering, which quotes the value it rejected.
            warn!(
                "Skipping an unreadable account frame ({} bytes): {:?} error at line {}, column {}",
                data.len(),
                e.classify(),
                e.line(),
                e.column()
            );
            debug!("account frame decode error: {e}");
            return None;
        }
    };

    let kind = frame
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let action = frame
        .get("action")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    if let Some(kind) = kind {
        return Some(decode_notification(kind, &frame));
    }

    // No `type`, so it is an acknowledgement or a refusal. The venue
    // distinguishes them with `status`.
    if let Some(status) = frame.get("status").and_then(serde_json::Value::as_str) {
        let decoded = if status.eq_ignore_ascii_case("error") {
            serde_json::from_value::<ErrorMessage>(frame.clone()).map(AccountEvent::ErrorMessage)
        } else {
            serde_json::from_value::<StatusMessage>(frame.clone()).map(AccountEvent::StatusMessage)
        };

        return Some(match decoded {
            Ok(event) => event,
            Err(e) => {
                // Still delivered. An acknowledgement this crate cannot shape
                // is worth less than one it can, and more than nothing.
                warn!(
                    "An account status frame did not match its shape ({} bytes, status {:?}): \
                     {:?} error at line {}, column {}",
                    data.len(),
                    status,
                    e.classify(),
                    e.line(),
                    e.column()
                );
                unknown_event(None, action, data)
            }
        });
    }

    debug!(
        "An account frame carried neither a type nor a status ({} bytes)",
        data.len()
    );
    Some(unknown_event(None, action, data))
}

/// Places one `type`d frame, keeping the payload whatever happens.
fn decode_notification(kind: String, frame: &serde_json::Value) -> AccountEvent {
    let timestamp = frame.get("timestamp").and_then(serde_json::Value::as_i64);
    // `data` is where the venue puts the object. A frame that has a `type` and
    // no `data` is still a notification; it just has nothing in it.
    let data = frame
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let payload = match kind.as_str() {
        "Order" => typed(&kind, &data, NotificationPayload::Order),
        "AccountBalance" => typed(&kind, &data, NotificationPayload::AccountBalance),
        "CurrentPosition" => typed(&kind, &data, NotificationPayload::CurrentPosition),
        "QuoteAlert" => typed(&kind, &data, NotificationPayload::QuoteAlert),
        "PublicWatchlists" => typed(&kind, &data, NotificationPayload::PublicWatchlist),
        other if OBSERVED_BUT_UNTYPED.contains(&other) => {
            debug!("Delivering an untyped {other} notification without decoding its payload");
            NotificationPayload::Unsupported(raw(&data))
        }
        other => {
            // A type the venue added. Naming it is safe; the payload is not,
            // so it travels in the event rather than in this line.
            debug!("Delivering an unrecognised {other} notification as an untyped payload");
            NotificationPayload::Unsupported(raw(&data))
        }
    };

    AccountEvent::Notification(Box::new(AccountNotification {
        kind,
        timestamp,
        payload,
    }))
}

/// Decodes `data` into `T`, falling back to the raw payload rather than
/// dropping the notification.
///
/// A model that has drifted from the wire is this crate's defect, and making
/// the caller lose a fill over it is the wrong trade. The type name and the
/// serde classification say what to fix; the payload goes to the caller.
fn typed<T, F>(kind: &str, data: &serde_json::Value, wrap: F) -> NotificationPayload
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(Box<T>) -> NotificationPayload,
{
    match serde_json::from_value::<T>(data.clone()) {
        Ok(value) => wrap(Box::new(value)),
        Err(e) => {
            warn!(
                "A {kind} notification did not match its model ({:?} error at line {}, column {}); \
                 delivering the payload untyped",
                e.classify(),
                e.line(),
                e.column()
            );
            debug!("{kind} payload decode error: {e}");
            NotificationPayload::Unsupported(raw(data))
        }
    }
}

/// The JSON text of a value, for a payload this crate is not modelling.
fn raw(data: &serde_json::Value) -> RawPayload {
    // Re-serialising a Value cannot fail for anything that came out of a
    // parse, but a library does not get to assume that: an empty payload is a
    // worse answer than a wrong one only if it pretends otherwise, and
    // `RawPayload` reports its own length.
    RawPayload::new(serde_json::to_string(data).unwrap_or_default())
}

/// An unknown frame, carrying the bytes exactly as they arrived.
fn unknown_event(kind: Option<String>, action: Option<String>, data: &[u8]) -> AccountEvent {
    AccountEvent::Unknown(UnknownEvent {
        kind,
        action,
        payload: RawPayload::new(String::from_utf8_lossy(data).into_owned()),
    })
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
mod credential_tests {
    use super::*;
    use crate::accounts::AccountNumber;

    /// The websocket takes the same `Bearer `-prefixed value as the HTTP
    /// header, and the whole request must be safe to format. A derived `Debug`
    /// rendered the live token.
    #[test]
    fn formatting_a_subscription_request_never_shows_the_token() {
        const TOKEN: &str = "SENTINEL-access-token-5Nd9";

        let message = SubRequest::<Vec<AccountNumber>> {
            auth_token: crate::oauth::AccessToken::new(TOKEN).bearer(),
            action: SubRequestAction::Connect,
            value: Some(vec![AccountNumber("SENTINEL-5WX00042".to_string())]),
        };

        // What goes on the wire still carries it, prefix included.
        let sent = serde_json::to_string(&message).expect("the request serializes");
        assert!(
            sent.contains(&format!(r#""auth-token":"Bearer {TOKEN}""#)),
            "the prefix is part of the credential: {sent}"
        );

        // What goes anywhere else does not.
        let rendered = format!("{message:?}");
        assert!(
            !rendered.contains(TOKEN),
            "the token reached Debug: {rendered}"
        );
        assert!(rendered.contains("***"), "{rendered}");
        assert!(
            rendered.contains("Connect"),
            "the action is safe: {rendered}"
        );
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
    ///
    /// The frame is now *delivered* rather than dropped — losing an order
    /// notification because one field drifted is the worse failure — but the
    /// log rule is unchanged.
    #[test]
    fn a_frame_that_does_not_match_its_model_never_logs_its_contents_at_warn() {
        // `status` wants a string; a number there makes serde quote the
        // neighbouring context, and the frame carries an account number.
        let frame = format!(
            r#"{{"type":"Order","data":{{"account-number":"{ACCOUNT_NUMBER}","status":12345}}}}"#
        );

        let (event, logs) = decode_capturing(frame.as_bytes(), Level::WARN);

        let Some(AccountEvent::Notification(notification)) = event else {
            panic!("a typed frame that does not decode is still a notification");
        };
        assert_eq!(notification.kind, "Order");
        let NotificationPayload::Unsupported(payload) = &notification.payload else {
            panic!("the payload could not be modelled, so it travels untyped");
        };
        assert!(
            payload.expose().contains(ACCOUNT_NUMBER),
            "the payload must reach the caller intact"
        );

        assert!(
            !logs.contains(ACCOUNT_NUMBER),
            "the account number reached the logs:\n{logs}"
        );
        assert!(
            logs.contains("error at line"),
            "the failure must still be diagnosable:\n{logs}"
        );
    }

    /// The payload is the caller's own account data, so they may read it — but
    /// only on purpose. Rendering it must cost nothing but a byte count.
    #[test]
    fn a_raw_payload_does_not_render_itself() {
        let payload = RawPayload::new(format!(r#"{{"account-number":"{ACCOUNT_NUMBER}"}}"#));

        for rendered in [format!("{payload:?}"), format!("{payload}")] {
            assert!(!rendered.contains(ACCOUNT_NUMBER), "{rendered}");
            assert!(rendered.contains("redacted"), "{rendered}");
            assert!(rendered.contains(&payload.len().to_string()), "{rendered}");
        }
        assert!(payload.expose().contains(ACCOUNT_NUMBER));
        assert!(!payload.is_empty());
    }

    #[test]
    fn the_detail_is_available_one_level_down() {
        let frame = br#"{ not json at all"#;
        let (event, logs) = decode_capturing(frame, Level::DEBUG);

        assert!(event.is_none(), "bytes that are not JSON are not an event");
        assert!(
            logs.contains("decode error"),
            "DEBUG keeps the full error:\n{logs}"
        );
    }
}

#[cfg(test)]
mod frame_routing_tests {
    use super::*;

    const ACCOUNT_NUMBER: &str = "SENTINEL-5WX00042";

    fn decode(frame: &str) -> AccountEvent {
        decode_account_frame(frame.as_bytes()).expect("valid JSON is always an event")
    }

    /// The documented `connect` notification, verbatim from the account
    /// streaming guide, with the account number replaced by a sentinel.
    ///
    /// The fills inside `legs` are the reason this matters: no REST endpoint
    /// in this crate returns an execution, so this frame is the only place a
    /// caller ever learns what they paid.
    #[test]
    fn the_documented_order_notification_decodes_with_its_fills() {
        let frame = format!(
            r#"{{
              "type": "Order",
              "data": {{
                "id": 1,
                "account-number": "{ACCOUNT_NUMBER}",
                "time-in-force": "Day",
                "order-type": "Market",
                "size": 100,
                "underlying-symbol": "AAPL",
                "underlying-instrument-type": "Equity",
                "price": "100.0",
                "price-effect": "Debit",
                "status": "Filled",
                "cancellable": false,
                "editable": false,
                "edited": false,
                "received-at": "2023-07-05T19:07:32.444+00:00",
                "updated-at": 1688584052750,
                "live-at": "2023-07-05T19:07:32.495+00:00",
                "terminal-at": "2023-07-05T19:07:32.737+00:00",
                "destination-venue": "TEST_A",
                "user-id": "99",
                "username": "coolperson",
                "legs": [
                  {{
                    "instrument-type": "Equity",
                    "symbol": "AAPL",
                    "quantity": 100,
                    "remaining-quantity": 0,
                    "action": "Buy to Open",
                    "fills": [
                      {{
                        "ext-group-fill-id": "0",
                        "ext-exec-id": "1122",
                        "fill-id": "24_TW::TEST_A47504::20230705.1179-TEST_FILL",
                        "quantity": 100,
                        "fill-price": "100.0",
                        "filled-at": "2023-07-05T19:07:32.496+00:00",
                        "destination-venue": "TEST_A"
                      }}
                    ]
                  }}
                ]
              }},
              "timestamp": 1688595114405
            }}"#
        );

        let AccountEvent::Notification(notification) = decode(&frame) else {
            panic!("a documented order notification must be a notification");
        };
        assert_eq!(notification.kind, "Order");
        assert_eq!(notification.timestamp, Some(1_688_595_114_405));

        let NotificationPayload::Order(order) = notification.payload else {
            panic!("the Order payload must be typed");
        };
        assert_eq!(order.legs.len(), 1, "the legs used to be discarded");
        let fill = &order.legs[0].fills[0];
        assert_eq!(
            fill.fill_price,
            Some(rust_decimal::Decimal::new(1000, 1)),
            "the fill price is the whole point of the frame"
        );
        assert_eq!(fill.destination_venue.as_deref(), Some("TEST_A"));
        assert!(fill.filled_at.is_some());
        // Two sources disagree about this one, so both shapes survive.
        assert_eq!(order.updated_at.as_deref(), Some("1688584052750"));
        assert!(order.received_at.is_some());
        assert!(order.reject_reason.is_none());
    }

    /// A market order has no price. The venue's own worked example is one,
    /// and a required `price` meant that notification — the commonest order
    /// type there is — could not be decoded at all.
    #[test]
    fn a_market_order_notification_without_a_price_decodes() {
        let frame = format!(
            r#"{{"type":"Order","data":{{
                 "id": 1,
                 "account-number": "{ACCOUNT_NUMBER}",
                 "time-in-force": "Day",
                 "order-type": "Market",
                 "size": 100,
                 "underlying-symbol": "AAPL",
                 "status": "Routed",
                 "cancellable": true,
                 "editable": true,
                 "edited": false,
                 "user-id": 99,
                 "leg-count": 1
               }}}}"#
        );

        let AccountEvent::Notification(notification) = decode(&frame) else {
            panic!("a market order is a notification");
        };
        let NotificationPayload::Order(order) = notification.payload else {
            panic!("a market order must be typed, not delivered raw");
        };
        assert!(order.price.is_none(), "a market order has no price");
        assert!(order.price_effect.is_none());
        // Numeric where the schema says string: both shapes reach the caller.
        assert_eq!(order.user_id.as_deref(), Some("99"));
        assert_eq!(order.leg_count.as_deref(), Some("1"));
    }

    /// The regression that made acknowledgements invisible: `request-id` is
    /// optional on the way out, this crate sends none, so none ever came back
    /// — and a required `u64` meant every status frame failed the untagged
    /// decode and was dropped with a warning.
    #[test]
    fn a_connect_acknowledgement_without_a_request_id_is_delivered() {
        let frame = format!(
            r#"{{"status":"ok","action":"connect","web-socket-session-id":"5b6e2799",
                 "value":["{ACCOUNT_NUMBER}"]}}"#
        );

        let AccountEvent::StatusMessage(status) = decode(&frame) else {
            panic!("an acknowledgement must reach the caller");
        };
        assert_eq!(status.action, "connect");
        assert_eq!(status.status, "ok");
        assert_eq!(status.request_id, None);
        assert_eq!(
            status.value.map(|accounts| accounts[0].0.clone()),
            Some(ACCOUNT_NUMBER.to_string()),
            "connect echoes what it subscribed"
        );
    }

    #[test]
    fn a_refusal_is_an_error_message() {
        let frame = r#"{"status":"error","action":"connect","web-socket-session-id":"5b6e2799",
                        "message":"connect-not-completed"}"#;

        let AccountEvent::ErrorMessage(error) = decode(frame) else {
            panic!("a refusal must be an error message");
        };
        assert_eq!(error.message, "connect-not-completed");
    }

    /// A notification type the venue adds tomorrow. It must reach the caller
    /// with its name and its payload rather than being dropped.
    #[test]
    fn an_unrecognised_type_arrives_as_an_untyped_payload() {
        let frame = format!(
            r#"{{"type":"SomethingNew","data":{{"account-number":"{ACCOUNT_NUMBER}"}},
                 "timestamp":1}}"#
        );

        let AccountEvent::Notification(notification) = decode(&frame) else {
            panic!("an unrecognised type is still a notification");
        };
        assert_eq!(notification.kind, "SomethingNew");
        let NotificationPayload::Unsupported(payload) = &notification.payload else {
            panic!("there is no model for it, so it travels untyped");
        };
        assert!(payload.expose().contains(ACCOUNT_NUMBER));
    }

    /// The variants the old code had arms for that carried no data at all:
    /// the payload was parsed and thrown away. They are preserved now,
    /// untyped, until a captured frame establishes a schema.
    #[test]
    fn an_observed_but_untyped_notification_keeps_its_payload() {
        for kind in OBSERVED_BUT_UNTYPED {
            let frame =
                format!(r#"{{"type":"{kind}","data":{{"account-number":"{ACCOUNT_NUMBER}"}}}}"#);

            let AccountEvent::Notification(notification) = decode(&frame) else {
                panic!("{kind} must still be a notification");
            };
            assert_eq!(notification.kind, kind);
            let NotificationPayload::Unsupported(payload) = &notification.payload else {
                panic!("{kind} has no captured frame, so it must not claim a type");
            };
            assert!(
                payload.expose().contains(ACCOUNT_NUMBER),
                "{kind} discarded its payload"
            );
        }
    }

    /// A frame that is neither typed nor a status message. Previously a decode
    /// failure and a dropped event.
    #[test]
    fn a_frame_that_is_neither_reaches_the_caller_as_unknown() {
        let AccountEvent::Unknown(unknown) = decode(r#"{"something":"else"}"#) else {
            panic!("an unplaceable frame must still be delivered");
        };
        assert_eq!(unknown.kind, None);
        assert!(unknown.payload.expose().contains("something"));
    }

    #[test]
    fn a_quote_alert_and_a_public_watchlist_are_typed() {
        let AccountEvent::Notification(alert) = decode(
            r#"{"type":"QuoteAlert","data":{"symbol":"AAPL","threshold-numeric":"200.00"}}"#,
        ) else {
            panic!("a quote alert is a notification");
        };
        assert!(matches!(alert.payload, NotificationPayload::QuoteAlert(_)));

        let AccountEvent::Notification(watchlist) = decode(
            r#"{"type":"PublicWatchlists","data":{"name":"High Options Volume",
                 "watchlist-entries":[{"symbol":"AAPL"}]}}"#,
        ) else {
            panic!("a watchlist is a notification");
        };
        let NotificationPayload::PublicWatchlist(list) = watchlist.payload else {
            panic!("the watchlist payload must be typed");
        };
        assert_eq!(list.watchlist_entries.len(), 1);
    }

    /// A `type` with no `data` is still a notification. Treating the missing
    /// payload as a decode failure would drop it.
    #[test]
    fn a_typed_frame_without_a_payload_is_still_delivered() {
        let AccountEvent::Notification(notification) = decode(r#"{"type":"OrderChain"}"#) else {
            panic!("a bare type is still a notification");
        };
        assert_eq!(notification.kind, "OrderChain");
        assert!(matches!(
            notification.payload,
            NotificationPayload::Unsupported(_)
        ));
    }
}
