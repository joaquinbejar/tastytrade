use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    accounts::{Account, Balance},
    Result, TastyTrade,
};

use super::{order::LiveOrderRecord, position::BriefPosition};

static WEBSOCKET_DEMO_URL: &str = "wss://streamer.cert.tastyworks.com";
static WEBSOCKET_URL: &str = "wss://streamer.tastyworks.com";

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

#[derive(Debug)]
pub struct AccountStreamer {
    pub event_receiver: flume::Receiver<AccountEvent>,
    pub action_sender: flume::Sender<HandlerAction>,
}

impl AccountStreamer {
    pub async fn connect(tasty: &TastyTrade) -> Result<AccountStreamer> {
        let token = &tasty.session_token;
        let (event_sender, event_receiver) = flume::unbounded();
        let (action_sender, action_receiver): (
            flume::Sender<HandlerAction>,
            flume::Receiver<HandlerAction>,
        ) = flume::unbounded();

        let url = if tasty.demo {
            url::Url::parse(WEBSOCKET_DEMO_URL).unwrap()
        } else {
            url::Url::parse(WEBSOCKET_URL).unwrap()
        };

        let (ws_stream, _response) = connect_async(url).await?;
        // let hello = ws_stream.try_next().await?;
        // if let Some(msg) = hello {
        //     match serde_json::from_slice(&msg.into_data())? {
        //         SubMessage::ErrorMessage(_) => return Err(ConnectionClosed.into()), // Perhaps retry on our own?
        //         SubMessage::StatusMessage(_) => {}
        //         _ => unreachable!(),
        //     }
        // } else {
        //     return Err(ConnectionClosed.into());
        // }

        let (mut write, mut read) = ws_stream.split();

        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                let data = message.unwrap().into_data();
                //println!("{:?}", String::from_utf8_lossy(&data));
                let data: AccountEvent = serde_json::from_slice(&data).unwrap();
                event_sender.send_async(data).await.unwrap();
            }
        });

        let token_clone = token.clone();
        tokio::spawn(async move {
            while let Ok(action) = action_receiver.recv_async().await {
                let message = SubRequest {
                    auth_token: token_clone.clone(),
                    action: action.action,
                    value: action.value,
                };
                let message = serde_json::to_string(&message).unwrap();

                //println!("{message:?}");

                let message = Message::Text(message);

                if write.send(message).await.is_err() {
                    // TODO: send message informing user of disconnection
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
        })
    }

    pub async fn subscribe_to_account<'a>(&self, account: &'a Account<'a>) {
        self.send(
            SubRequestAction::Connect,
            Some(vec![account.inner.account.account_number.clone()]),
        )
        .await;
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

    // pub async fn close(&self) {}

    pub async fn get_event(&self) -> std::result::Result<AccountEvent, flume::RecvError> {
        self.event_receiver.recv_async().await
    }
}

impl TastyTrade {
    pub async fn create_account_streamer(&self) -> Result<AccountStreamer> {
        AccountStreamer::connect(self).await
    }
}
