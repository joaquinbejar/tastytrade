//! A minimal HTTP/1.1 server standing in for the tastytrade REST API.
//!
//! Deliberately hand-rolled over `tokio::net::TcpListener` rather than pulling
//! in a mock-server crate: the suite needs to serve a handful of canned
//! responses and to be able to produce malformed ones on purpose, and a new
//! runtime dependency would need its own approval.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// What the venue answers for one `METHOD /path` pair.
#[derive(Clone, Debug)]
pub struct Route {
    /// HTTP status to send.
    pub status: u16,
    /// Raw body. Not required to be valid JSON — malformed responses are part
    /// of what this suite exercises.
    pub body: String,
}

impl Route {
    /// A `200 OK` carrying `body`.
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }

    /// A failing response carrying `body`.
    pub fn status(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }
}

/// One request the venue received, kept so a test can assert on what the
/// client actually sent.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    /// e.g. `GET`.
    pub method: String,
    /// Path with query string, e.g. `/accounts/5WX1/balances?x=1`.
    pub target: String,
    /// Request body, empty for verbs that carry none.
    pub body: String,
    /// Request headers, lowercased names to values.
    pub headers: HashMap<String, String>,
}

/// A loopback stand-in for the tastytrade REST API.
///
/// Bound to port 0, so many can run concurrently and no test needs a fixed
/// port. The listener task stops when the value is dropped.
pub struct MockVenue {
    base_url: String,
    received: Arc<Mutex<Vec<RecordedRequest>>>,
    handle: JoinHandle<()>,
}

impl MockVenue {
    /// Starts a venue that answers `routes`, keyed by `"METHOD /path"`.
    ///
    /// A request with no matching route gets `404` and a JSON error body, so an
    /// unexpected call fails the test loudly instead of hanging.
    pub async fn start(routes: HashMap<String, Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback port must be available");
        let addr = listener
            .local_addr()
            .expect("the listener must have an addr");
        let base_url = format!("http://{addr}");

        let received = Arc::new(Mutex::new(Vec::new()));
        let recorder = received.clone();

        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                let recorder = recorder.clone();

                tokio::spawn(async move {
                    let mut raw: Vec<u8> = Vec::new();
                    let mut buf = [0u8; 4096];

                    // Framing is done on the raw bytes throughout: Content-Length
                    // counts bytes, and deciding when the body is complete from a
                    // lossily decoded String would wait for bytes that already
                    // arrived, or slice at the wrong boundary, as soon as
                    // anything non-ASCII is in play. Only the finished body is
                    // decoded, and only for assertions.
                    let (method, target, headers, body) = loop {
                        let Ok(read) = socket.read(&mut buf).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        raw.extend_from_slice(&buf[..read]);

                        let Some(header_end) =
                            raw.windows(4).position(|window| window == b"\r\n\r\n")
                        else {
                            continue;
                        };
                        let body_start = header_end + 4;

                        // Headers are ASCII by definition, so decoding the head
                        // is safe; the body is what needed the care.
                        let head = String::from_utf8_lossy(&raw[..header_end]).into_owned();
                        let mut lines = head.split("\r\n");
                        let request_line = lines.next().unwrap_or_default();
                        let mut parts = request_line.split_whitespace();
                        let method = parts.next().unwrap_or_default().to_string();
                        let target = parts.next().unwrap_or_default().to_string();

                        let headers: HashMap<String, String> = lines
                            .filter_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
                            })
                            .collect();

                        let content_length = headers
                            .get("content-length")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(0);

                        if raw.len() - body_start >= content_length {
                            let body = String::from_utf8_lossy(
                                &raw[body_start..body_start + content_length],
                            )
                            .into_owned();
                            break (method, target, headers, body);
                        }
                    };

                    recorder
                        .lock()
                        .expect("the recorder mutex is never poisoned in tests")
                        .push(RecordedRequest {
                            method: method.clone(),
                            target: target.clone(),
                            body,
                            headers,
                        });

                    // Route on the path, ignoring any query string.
                    let path = target.split('?').next().unwrap_or(&target);
                    let route = routes.get(&format!("{method} {path}")).cloned().unwrap_or(
                        Route::status(
                            404,
                            r#"{"error":{"code":"not_found","message":"no route in the mock venue"}}"#,
                        ),
                    );

                    let response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        route.status,
                        reason(route.status),
                        route.body.len(),
                        route.body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        Self {
            base_url,
            received,
            handle,
        }
    }

    /// Convenience for a venue that only needs to answer a successful login.
    pub async fn with_login(body: String) -> Self {
        let mut routes = HashMap::new();
        routes.insert("POST /sessions".to_string(), Route::ok(body));
        Self::start(routes).await
    }

    /// `http://127.0.0.1:<port>`, ready to drop into `TastyTradeConfig`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.received
            .lock()
            .expect("the recorder mutex is never poisoned in tests")
            .clone()
    }
}

impl Drop for MockVenue {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}
