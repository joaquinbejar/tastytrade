use std::time::Duration;

use crate::types::balance::Balance;
use crate::{
    BriefPosition, LiveOrderRecord, TastyResult, TastyTrade, TastyTradeError, accounts::Account,
};
use dxlink::{DXLinkClient, EventType, FeedSubscription};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
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

/// Represents a subscription request.
///
/// This struct is used to send subscription requests to the server.
/// The `value` field is optional and its type depends on the `action` field.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SubRequest<T> {
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
#[derive(Deserialize, Debug)]
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
#[derive(Deserialize, Debug)]
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

/**
Represents a command that can be sent to a DXLink service.

This enum defines the different types of commands that can be used to interact with a DXLink service,
primarily for managing subscriptions to data feeds.
*/
enum DXLinkCommand {
    /// Subscribes to a set of data feeds.
    ///
    /// The first parameter is a unique request ID (u32). The DXLink service should respond with this same ID.
    /// The second parameter is a vector of `FeedSubscription`s, defining the feeds to subscribe to.
    Subscribe(u32, Vec<FeedSubscription>),

    /// Unsubscribes from a set of data feeds.
    ///
    /// The first parameter is a unique request ID (u32). The DXLink service should respond with this same ID.
    /// The second parameter is a vector of `FeedSubscription`s, defining the feeds to unsubscribe from.
    #[allow(dead_code)]
    Unsubscribe(u32, Vec<FeedSubscription>),

    /// Disconnects from the DXLink service.
    Disconnect,
}

/// AccountStreamer struct.
///
/// Provides a way to stream account events. Uses DXLink for communication.
///
#[derive(Debug)]
pub struct AccountStreamer {
    /// Receiver for account events.
    pub event_receiver: flume::Receiver<AccountEvent>,
    /// Sender for actions to be handled.
    pub action_sender: flume::Sender<HandlerAction>,
    /// Optional channel ID for DXLink communication.
    channel_id: Option<u32>,
    /// Optional sender for DXLink commands.
    dxlink_command_tx: Option<mpsc::Sender<DXLinkCommand>>,
}

impl AccountStreamer {
    /// Establishes a connection to the TastyTrade streaming API for account updates.
    ///
    /// This function initializes and manages two separate streaming connections:
    /// 1. **DXLink:** A newer, more robust streaming solution.  It attempts to create and configure a DXLink channel for account updates, subscribing to `Order` and `Message` event types.  If successful, it uses this channel for streaming data.  If DXLink setup fails, it falls back to the legacy websocket implementation.
    /// 2. **Legacy Websocket:**  A fallback mechanism used if DXLink connection or channel setup fails. It maintains a persistent websocket connection to receive account updates.
    ///
    /// Both implementations handle incoming messages and send outgoing actions (e.g., heartbeats, subscriptions).  The DXLink implementation also includes a command channel for managing subscriptions and disconnections.
    ///
    /// # Arguments
    ///
    /// * `tasty` - A reference to the `TastyTrade` client, containing authentication and configuration details.
    ///
    /// # Returns
    ///
    /// * `Ok(AccountStreamer)` - If the connection is successful, returns an `AccountStreamer` instance for managing the stream.
    /// * `Err(TastyTradeError)` - If an error occurs during connection or setup.  This could be due to network issues, invalid credentials, or problems with the DXLink or legacy websocket connection.
    ///
    /// # Errors
    ///
    /// This function can return a variety of errors related to network communication, authentication, or streaming setup. See the `TastyTradeError` enum for more details.
    pub async fn connect(tasty: &TastyTrade) -> TastyResult<AccountStreamer> {
        let token = &tasty.session_token;
        let (event_sender, event_receiver) = flume::unbounded();
        let (action_sender, action_receiver): (
            flume::Sender<HandlerAction>,
            flume::Receiver<HandlerAction>,
        ) = flume::unbounded();

        // Initialize DXLink client for account updates
        let mut client = DXLinkClient::new(&tasty.config.websocket_url, token);

        // Connect to DXLink
        match client.connect().await {
            Ok(_) => debug!("Connected to DXLink for account updates"),
            Err(e) => {
                warn!("Error connecting to DXLink for account updates: {}", e);
                return Err(TastyTradeError::Streaming(format!(
                    "Error connecting to DXLink for account updates: {}",
                    e
                )));
            }
        }

        // Create channel for account data
        let channel_id = match client.create_feed_channel("ACCOUNT").await {
            Ok(id) => {
                debug!("Created DXLink channel {} for account updates", id);
                Some(id)
            }
            Err(e) => {
                warn!(
                    "Could not create DXLink channel for account, using legacy implementation: {}",
                    e
                );
                None
            }
        };

        // Configure channel if created successfully
        if let Some(id) = channel_id {
            match client
                .setup_feed(id, &[EventType::Order, EventType::Message])
                .await
            {
                Ok(_) => debug!("Successfully set up DXLink feed for account"),
                Err(e) => warn!("Error setting up DXLink feed for account: {}", e),
            }
        }

        // Create command channel for DXLink operations
        let (command_tx, mut command_rx) = mpsc::channel::<DXLinkCommand>(100);

        // Spawn task to handle DXLink commands
        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    DXLinkCommand::Subscribe(channel_id, subscriptions) => {
                        match client.subscribe(channel_id, subscriptions).await {
                            Ok(_) => debug!("Successfully subscribed to account via DXLink"),
                            Err(e) => warn!("Error subscribing to account via DXLink: {}", e),
                        }
                    }
                    DXLinkCommand::Unsubscribe(channel_id, subscriptions) => {
                        match client.unsubscribe(channel_id, subscriptions).await {
                            Ok(_) => debug!("Successfully unsubscribed from account via DXLink"),
                            Err(e) => warn!("Error unsubscribing from account via DXLink: {}", e),
                        }
                    }
                    DXLinkCommand::Disconnect => {
                        match client.disconnect().await {
                            Ok(_) => debug!("Successfully disconnected DXLink account client"),
                            Err(e) => warn!("Error disconnecting DXLink account client: {}", e),
                        }
                        break; // Exit the loop after disconnecting
                    }
                }
            }
            debug!("DXLink account command handler terminated");
        });

        // Keep existing tokio-tungstenite implementation for compatibility
        let url = tasty.config.websocket_url.clone();
        let token_clone = token.clone();

        let (ws_stream, _response) = connect_async(url).await?;

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
                let message = SubRequest {
                    auth_token: token_clone.clone(),
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
            channel_id,
            dxlink_command_tx: Some(command_tx),
        })
    }

    /// Subscribes to account updates.
    ///
    /// This function subscribes to updates for the given account. It uses two methods for subscribing:
    /// 1. It sends a `Connect` message with the account number to the internal `action_sender`.
    /// 2. If DXLink is configured (`dxlink_command_tx` and `channel_id` are not `None`), it also sends a `Subscribe` command
    ///    to the DXLink client, subscribing to "Order" and "Message" events for the account.
    ///
    /// # Arguments
    ///
    /// * `account` - A reference to the `Account` object to subscribe to.
    ///
    pub async fn subscribe_to_account<'a>(&self, account: &'a Account<'a>) {
        self.send(
            SubRequestAction::Connect,
            Some(vec![account.inner.account.account_number.clone()]),
        )
        .await;

        // If we have DXLink configured, also subscribe through that channel
        if let (Some(tx), Some(ch_id)) = (&self.dxlink_command_tx, self.channel_id) {
            // Subscribe to updates for specific account
            let account_number = account.inner.account.account_number.0.clone();
            let subscriptions = vec![
                FeedSubscription {
                    event_type: "Order".to_string(),
                    symbol: account_number.clone(),
                    from_time: None,
                    source: None,
                },
                FeedSubscription {
                    event_type: "Message".to_string(),
                    symbol: account_number,
                    from_time: None,
                    source: None,
                },
            ];

            let tx_clone = tx.clone();
            let channel_id = ch_id;

            tokio::spawn(async move {
                if let Err(e) = tx_clone
                    .send(DXLinkCommand::Subscribe(channel_id, subscriptions))
                    .await
                {
                    error!("Error sending account subscription command: {}", e);
                }
            });
        }
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

impl Drop for AccountStreamer {
    /// Cleans up resources when the `AccountStreamer` is dropped.
    ///
    /// This implementation sends a `Disconnect` command to the DXLink client
    /// if a command channel is available.  This ensures a clean disconnect
    /// from the data stream.  The disconnect command is sent asynchronously
    /// to avoid blocking the drop function.  Any errors encountered while
    /// sending the disconnect command are logged as warnings.
    fn drop(&mut self) {
        // Send disconnect command if we have a command channel
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

impl TastyTrade {
    /// Creates a new `AccountStreamer`.
    ///
    /// This function attempts to establish a connection to the TastyTrade streaming API
    /// for account updates.  It prioritizes using DXLink, a newer and more robust
    /// streaming solution. If DXLink connection fails, it falls back to a legacy
    /// websocket implementation.
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
