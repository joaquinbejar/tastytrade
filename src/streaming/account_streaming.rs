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

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubRequestAction {
    Heartbeat,
    Connect,
    PublicWatchlistsSubscribe,
    QuoteAlertsSubscribe,
    UserMessageSubscribe,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct SubRequest<T> {
    auth_token: String,
    action: SubRequestAction,
    value: Option<T>,
}

pub struct HandlerAction {
    action: SubRequestAction,
    value: Option<Box<dyn erased_serde::Serialize + Send + Sync>>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum AccountMessage {
    Order(LiveOrderRecord),
    AccountBalance(Box<Balance>),
    CurrentPosition(Box<BriefPosition>),
    OrderChain,
    ExternalTransaction,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct StatusMessage {
    pub status: String,
    pub action: String,
    pub web_socket_session_id: String,
    pub request_id: u64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct ErrorMessage {
    pub status: String,
    pub action: String,
    pub web_socket_session_id: String,
    pub message: String,
}

//#[allow(clippy::large_enum_variant)]
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum AccountEvent {
    ErrorMessage(ErrorMessage),
    StatusMessage(StatusMessage),
    AccountMessage(Box<AccountMessage>),
}

// Commands for DXLink client
enum DXLinkCommand {
    Subscribe(u32, Vec<FeedSubscription>),
    #[allow(dead_code)]
    Unsubscribe(u32, Vec<FeedSubscription>),
    Disconnect,
}

#[derive(Debug)]
pub struct AccountStreamer {
    pub event_receiver: flume::Receiver<AccountEvent>,
    pub action_sender: flume::Sender<HandlerAction>,

    // New implementation with DXLink
    channel_id: Option<u32>,
    dxlink_command_tx: Option<mpsc::Sender<DXLinkCommand>>,
}

impl AccountStreamer {
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

    pub async fn subscribe_to_account<'a>(&self, account: &'a Account<'a>) {
        // Existing implementation
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

    pub async fn get_event(&self) -> std::result::Result<AccountEvent, flume::RecvError> {
        self.event_receiver.recv_async().await
    }
}

impl Drop for AccountStreamer {
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
    pub async fn create_account_streamer(&self) -> TastyResult<AccountStreamer> {
        AccountStreamer::connect(self).await
    }
}
