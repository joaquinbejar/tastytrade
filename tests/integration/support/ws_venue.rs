//! A loopback websocket standing in for the tastytrade account streamer.
//!
//! The REST side has had `MockVenue` since the suite existed; the account
//! websocket had nothing, which is how a reconnect that could never work
//! survived three PRs through that file. This is the missing half.
//!
//! Hand-rolled over `tokio::net::TcpListener` and `tokio_tungstenite::accept_async`
//! for the same reason `MockVenue` is: the suite needs a handful of canned
//! answers and the ability to drop a connection on purpose, and a mock-server
//! crate would be a new dependency to justify.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

/// One frame the venue received, already parsed.
#[derive(Clone, Debug)]
pub struct RecordedFrame {
    /// Which connection it arrived on, counting from one. This is what tells a
    /// restoration apart from the original subscription.
    pub connection: usize,
    /// The `action` field.
    pub action: String,
    /// The `value` field rendered as text, which for `connect` is the account
    /// list.
    pub value: String,
}

/// A loopback stand-in for the account websocket.
///
/// Answers every action with `status: ok`, echoing the `request-id` it was
/// given — the behaviour the venue documents, and the one this crate now
/// depends on to know an action was accepted.
pub struct WsVenue {
    url: String,
    received: Arc<Mutex<Vec<RecordedFrame>>>,
    connections: Arc<AtomicUsize>,
    /// While set, a `connect` on a reconnected session is recorded but not
    /// answered.
    ///
    /// It exists so a test can look at the client *between* the request and
    /// the acknowledgement. Without it a test can only assert that a state
    /// eventually appears, which passes just as well when the state was
    /// claimed too early — exactly the bug worth catching here.
    hold_restoration: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl WsVenue {
    /// Starts a venue that drops the **first** connection after
    /// `frames_before_close` frames, forcing one reconnect, and then keeps
    /// every later connection open.
    ///
    /// Only the first, because a venue that dropped every connection would
    /// leave the client permanently reconnecting — and a test asserting on the
    /// state it reaches would be racing a socket that dies again immediately.
    /// One drop followed by a healthy connection is also the case that
    /// actually happens.
    ///
    /// `usize::MAX` keeps every connection open for the life of the test.
    pub async fn start(frames_before_close: usize) -> Self {
        Self::start_with(frames_before_close, false, false).await
    }

    /// The same, but a `connect` arriving on a reconnected session is left
    /// unanswered until [`WsVenue::release_restoration`].
    pub async fn holding_restoration(frames_before_close: usize) -> Self {
        Self::start_with(frames_before_close, true, false).await
    }

    /// A venue that accepts the socket and then refuses every restoration.
    ///
    /// The accept-then-reject shape the backoff exists to bound: without it a
    /// client that counts a successful write as a working session resets its
    /// attempt budget on every reconnect and retries forever.
    pub async fn refusing_restoration(frames_before_close: usize) -> Self {
        Self::start_with(frames_before_close, false, true).await
    }

    async fn start_with(
        frames_before_close: usize,
        hold_restoration: bool,
        refuse_restoration: bool,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port must be available");
        let addr = listener
            .local_addr()
            .expect("the listener must have an addr");
        let url = format!("ws://{addr}");

        let received = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let hold_restoration = Arc::new(AtomicBool::new(hold_restoration));
        let recorder = received.clone();
        let counter = connections.clone();
        let held = hold_restoration.clone();

        let handle = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let Ok(mut stream) = tokio_tungstenite::accept_async(socket).await else {
                    continue;
                };

                let connection = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let mut handled = 0usize;

                while let Some(Ok(message)) = stream.next().await {
                    let Message::Text(text) = message else {
                        continue;
                    };
                    let Ok(frame) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };

                    let action = frame
                        .get("action")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let value = frame
                        .get("value")
                        .map(ToString::to_string)
                        .unwrap_or_default();

                    recorder
                        .lock()
                        .expect("the recorder mutex is never poisoned in tests")
                        .push(RecordedFrame {
                            connection,
                            action: action.clone(),
                            value,
                        });

                    // A restoration the test is holding: recorded, and left
                    // unanswered until it says otherwise.
                    while connection > 1 && action == "connect" && held.load(Ordering::SeqCst) {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }

                    let refusing = refuse_restoration && connection > 1 && action == "connect";

                    // Echo the id back, which is what the venue documents and
                    // what turns a socket write into a confirmed action.
                    //
                    // Built with `json!` rather than `format!`: `action` comes
                    // from the client under test, and a hand-built string would
                    // emit invalid JSON for any value needing escaping. The
                    // client would then ignore the reply, and the test would
                    // fail as a timeout with nothing pointing at the cause.
                    let mut reply = if refusing {
                        serde_json::json!({
                            "status": "error",
                            "action": action,
                            "web-socket-session-id": "test",
                            "message": "connect-not-completed",
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "action": action,
                            "web-socket-session-id": "test",
                        })
                    };
                    if let Some(id) = frame.get("request-id")
                        && let Some(object) = reply.as_object_mut()
                    {
                        object.insert("request-id".to_string(), id.clone());
                    }
                    let reply = reply.to_string();

                    if stream.send(Message::Text(reply.into())).await.is_err() {
                        break;
                    }

                    handled += 1;
                    if connection == 1 && handled >= frames_before_close {
                        // The drop is the point: it is what makes the client
                        // reconnect and replay.
                        break;
                    }
                }
            }
        });

        Self {
            url,
            received,
            connections,
            hold_restoration,
            handle,
        }
    }

    /// Answers the restoration this venue has been holding.
    pub fn release_restoration(&self) {
        self.hold_restoration.store(false, Ordering::SeqCst);
    }

    /// `ws://127.0.0.1:<port>`, ready to drop into `TastyTradeConfig`.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Every frame received so far, in order.
    pub fn frames(&self) -> Vec<RecordedFrame> {
        self.received
            .lock()
            .expect("the recorder mutex is never poisoned in tests")
            .clone()
    }

    /// How many connections have been accepted.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The actions seen on `connection`, in order.
    pub fn actions_on(&self, connection: usize) -> Vec<String> {
        self.frames()
            .into_iter()
            .filter(|frame| frame.connection == connection)
            .map(|frame| frame.action)
            .collect()
    }

    /// Waits until `predicate` holds, or gives up.
    ///
    /// Polling rather than a channel because what the tests wait on is a
    /// supervisor's own progress, which has no signal to subscribe to — and a
    /// bounded wait turns a hang into a failure with a message.
    pub async fn wait_for(
        &self,
        what: &str,
        predicate: impl Fn(&Self) -> bool,
    ) -> Vec<RecordedFrame> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            if predicate(self) {
                return self.frames();
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!(
            "timed out waiting for {what}; saw {:?} across {} connection(s)",
            self.frames(),
            self.connections()
        );
    }
}

impl Drop for WsVenue {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// A `GET /customers/me/accounts` body with one healthy account.
pub fn one_account_body(account_number: &str) -> String {
    format!(
        r#"{{
            "data": {{
                "items": [
                    {{
                        "account": {{
                            "account-number": "{account_number}",
                            "nickname": "Test",
                            "account-type-name": "Individual",
                            "margin-or-cash": "Margin",
                            "opened-at": "2025-01-14T10:22:41.000+00:00"
                        }},
                        "authority-level": "owner"
                    }}
                ]
            }},
            "context": "/customers/me/accounts"
        }}"#
    )
}
