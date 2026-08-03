//! Shows what happens to a frame this crate does not model.
//!
//! Two halves. The first needs no network: it feeds frames straight to the
//! decoder, including a notification type that does not exist, so the
//! behaviour is visible without waiting for the venue to invent one. The
//! second connects and reports anything that arrives untyped, which is the
//! signal that a new notification type is worth adding.
//!
//! The old decoder dropped every one of these with a warning. On this socket a
//! dropped frame is a fill the caller never hears about.
//!
//! ```shell
//! cargo run -p accounts-status --bin stream_unknown_events
//! ```

use std::time::Duration;

use tastytrade::prelude::*;
use tokio::time::timeout;
use tracing::info;

const MAX_EVENTS: usize = 20;
const DEADLINE: Duration = Duration::from_secs(20);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    // ---- offline: no credentials, no socket ----
    println!("frames the decoder places without a connection:");
    for frame in [
        // A notification type the venue has not invented yet.
        r#"{"type":"SomethingNew","data":{"note":"kept, not dropped"},"timestamp":1}"#,
        // A type this crate knows exists but has no captured frame for.
        r#"{"type":"OrderChain","data":{"id":7}}"#,
        // Neither a notification nor an acknowledgement.
        r#"{"unexpected":"shape"}"#,
    ] {
        match AccountStreamer::decode_frame(frame.as_bytes()) {
            Some(AccountEvent::Notification(notification)) => println!(
                "  {} -> {}",
                notification.kind,
                match &notification.payload {
                    NotificationPayload::Unsupported(payload) =>
                        format!("untyped payload, {} bytes", payload.len()),
                    typed => format!("{typed:?}"),
                }
            ),
            Some(AccountEvent::Unknown(unknown)) => println!(
                "  unplaceable -> kind {:?}, {} bytes kept",
                unknown.kind,
                unknown.payload.len()
            ),
            Some(other) => println!("  {other:?}"),
            None => println!("  not JSON at all, so there is nothing to deliver"),
        }
    }

    // ---- live: report anything the venue sends that is not modelled ----
    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("No credentials configured; the offline half above is the whole example");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let streamer = tasty.create_account_streamer().await?;
    for account in &tasty.accounts().await? {
        streamer.subscribe_to_account(account).await?;
    }

    println!("\nlistening for untyped frames from the venue:");
    for _ in 0..MAX_EVENTS {
        let Ok(event) = timeout(DEADLINE, streamer.get_event()).await else {
            info!("Nothing further within {DEADLINE:?}; stopping");
            break;
        };

        match event? {
            AccountEvent::Notification(notification) => match notification.payload {
                NotificationPayload::Unsupported(payload) => println!(
                    "  {} arrived untyped ({} bytes) — worth modelling",
                    notification.kind,
                    payload.len()
                ),
                _ => println!("  {} arrived typed", notification.kind),
            },
            AccountEvent::Unknown(unknown) => println!(
                "  an unplaceable frame arrived: kind {:?}, action {:?}",
                unknown.kind, unknown.action
            ),
            _ => {}
        }
    }

    Ok(())
}
