//! Records real account frames from certification as test fixtures.
//!
//! The fixtures in `Doc/frames/account/` are *documented* or *derived* — copied
//! from the venue's guide, or assembled from the published swagger. A derived
//! frame fed to a type derived from the same swagger mostly proves the two
//! agree with each other. This is how they get replaced with the real thing.
//!
//! ```shell
//! cargo run -p accounts-status --bin capture_frames
//! ```
//!
//! Writes one `<type>.captured.json` per notification type it sees, into
//! `Doc/frames/account/`, with account numbers and user identifiers replaced.
//! It **does not** commit anything and does not overwrite a `.derived` file:
//! a fixture is evidence, and evidence gets read before it is checked in.
//!
//! Certification only. It refuses to run against production, where the frames
//! would be about a funded account.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use accounts_status::{frame_name, redact};
use tastytrade::prelude::*;
use tokio::time::timeout;
use tracing::info;

/// Where the fixtures live, relative to the repository root.
const FIXTURES: &str = "Doc/frames/account";

/// Long enough for a quiet account to produce balances and positions on
/// subscription; short enough to be run rather than left going.
const DEADLINE: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    // A captured frame from production is a frame about a funded account, and
    // it would be committed. The check is here rather than in a comment.
    if config.environment() != Environment::Certification {
        info!(
            "Refusing to capture from {}: these frames get committed, and a production \
             frame is about a real account. Set TASTYTRADE_USE_DEMO=true.",
            config.environment()
        );
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let streamer = tasty.create_account_streamer().await?;

    for account in &tasty.accounts().await? {
        streamer.subscribe_to_account(account).await?;
        info!("Subscribed to account {}", account.number().redacted());
    }
    streamer
        .send(
            SubRequestAction::PublicWatchlistsSubscribe,
            None::<Vec<String>>,
        )
        .await?;
    streamer
        .send(SubRequestAction::QuoteAlertsSubscribe, None::<Vec<String>>)
        .await?;

    info!("Listening for {DEADLINE:?}; one file per notification type");

    // One frame per type: the first is as good as the tenth for a fixture, and
    // a map keeps the output deterministic to read.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let deadline = tokio::time::Instant::now() + DEADLINE;

    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(event)) = timeout(Duration::from_secs(5), streamer.get_event()).await else {
            continue;
        };

        // Back to JSON through the raw payload where there is one, so what is
        // written is the frame rather than this crate's rendering of it.
        let Some(raw) = raw_json(&event) else {
            continue;
        };
        let Ok(mut frame) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };

        redact(&mut frame);
        let name = frame_name(&frame);
        let pretty = serde_json::to_string_pretty(&frame)?;
        seen.entry(name).or_insert(pretty);
    }

    if seen.is_empty() {
        info!("Nothing arrived. A quiet certification account is the normal case;");
        info!("place a dry-run order or move a position to produce order frames.");
        return Ok(());
    }

    let directory = PathBuf::from(FIXTURES);
    for (name, body) in &seen {
        let path = directory.join(format!("{name}.captured.json"));
        std::fs::write(&path, format!("{body}\n"))?;
        println!("wrote {}", path.display());
    }

    println!(
        "\n{} frame(s) captured. Before committing any of them:",
        seen.len()
    );
    println!("  1. read every file and check the redaction actually did its job —");
    println!("     it replaces the fields it knows, and the venue can add more");
    println!("  2. delete the .derived fixture each one supersedes");
    println!("  3. point the test in src/streaming/account_streaming.rs at the new name");
    println!("  4. reconcile anything that disagrees with the model — that is the point");

    Ok(())
}

/// The frame as JSON, for the events that carry one.
///
/// `Unsupported` and `Unknown` keep the bytes, which is exactly what a capture
/// wants. A typed payload has already been through a model, so re-serialising
/// it would record this crate's shape rather than the venue's — those are
/// skipped and reported instead.
fn raw_json(event: &AccountEvent) -> Option<String> {
    match event {
        AccountEvent::Unknown(unknown) => Some(unknown.payload.expose().to_string()),
        AccountEvent::Notification(notification) => match &notification.payload {
            NotificationPayload::Unsupported(payload) => Some(format!(
                r#"{{"type":"{}","data":{}}}"#,
                notification.kind,
                payload.expose()
            )),
            _ => {
                info!(
                    "A {} arrived and was decoded by the model, so its bytes are gone; \
                     capture it with a debug proxy if the exact frame matters",
                    notification.kind
                );
                None
            }
        },
        // Acknowledgements and refusals are small and worth having exactly.
        AccountEvent::StatusMessage(status) => serde_json::to_string(status).ok(),
        AccountEvent::ErrorMessage(error) => serde_json::to_string(error).ok(),
    }
}
