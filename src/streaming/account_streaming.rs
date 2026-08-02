use std::time::Duration;

use crate::types::balance::Balance;
use crate::{BriefPosition, LiveOrderRecord, TastyResult, TastyTrade, accounts::Account};
use futures_util::{SinkExt, StreamExt};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::debug;

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
        let writer_token = tasty.session_token.clone();
        let (event_sender, event_receiver) = flume::unbounded();
        let (action_sender, action_receiver): (
            flume::Sender<HandlerAction>,
            flume::Receiver<HandlerAction>,
        ) = flume::unbounded();

        let (ws_stream, _response) = connect_async(tasty.config.websocket_url.clone()).await?;
        debug!("Account websocket connected");

        let (mut write, mut read) = ws_stream.split();

        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                let data = message.unwrap().into_data();
                let data: AccountEvent = serde_json::from_slice(&data).unwrap();
                event_sender.send_async(data).await.unwrap();
            }
        });

        tokio::spawn(async move {
            while let Ok(action) = action_receiver.recv_async().await {
                let message = SubRequest::<Box<dyn erased_serde::Serialize + Send + Sync>> {
                    auth_token: writer_token.clone(),
                    action: action.action,
                    value: action.value,
                };
                let message = serde_json::to_string(&message).unwrap();
                let message = Message::Text(message.into());

                if write.send(message).await.is_err() {
                    break;
                }
            }
        });

        let sender_clone = action_sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if sender_clone
                    .send_async(HandlerAction {
                        action: SubRequestAction::Heartbeat,
                        value: None,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        Ok(Self {
            event_receiver,
            action_sender,
            transport: AccountTransport::Websocket,
        })
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
    pub async fn subscribe_to_account<'a>(&self, account: &'a Account<'a>) {
        self.send(
            SubRequestAction::Connect,
            Some(vec![account.inner.account.account_number.clone()]),
        )
        .await;
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
    ) {
        self.action_sender
            .send_async(HandlerAction {
                action,
                value: value
                    .map(|inner| Box::new(inner) as Box<dyn erased_serde::Serialize + Send + Sync>),
            })
            .await
            .unwrap();
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
