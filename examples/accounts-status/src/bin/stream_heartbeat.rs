//! Watches the heartbeat and the connection state.
//!
//! The streamer sends a heartbeat every thirty seconds on its own; this sends
//! one explicitly and then reports [`ConnectionState`] on a timer, which is
//! what tells a reconnect that is happening apart from one that is not.
//!
//! The reconnect path refreshes the OAuth access token rather than logging in
//! again — there is no username and password to fall back on any more — so a
//! long run here is also the cheapest way to watch a token roll over.
//!
//! ```shell
//! cargo run -p accounts-status --bin stream_heartbeat
//! ```

use std::time::Duration;

use tastytrade::prelude::*;
use tokio::time::{sleep, timeout};
use tracing::info;

/// Long enough to outlive one heartbeat interval, short enough to run in CI.
const RUN_FOR: Duration = Duration::from_secs(45);
const POLL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let streamer = tasty.create_account_streamer().await?;

    for account in &tasty.accounts().await? {
        streamer.subscribe_to_account(account).await?;
    }

    // Reaching the queue is not reaching the venue, so this waits for the
    // writer to say what happened.
    streamer
        .send(SubRequestAction::Heartbeat, None::<Vec<String>>)
        .await?;
    println!("heartbeat accepted by the venue");

    let deadline = tokio::time::Instant::now() + RUN_FOR;
    while tokio::time::Instant::now() < deadline {
        // The state carries counts and durations only — no token, no account —
        // so it is safe to print anywhere.
        println!("connection state: {:?}", streamer.state().await);
        // Non-secret expiry metadata. The token itself never leaves the crate.
        println!(
            "access token valid for {:?}",
            tasty.session().expires_in().await
        );

        // Drain whatever arrived rather than blocking on a quiet account.
        while let Ok(Ok(event)) = timeout(Duration::from_millis(50), streamer.get_event()).await {
            if let AccountEvent::StatusMessage(status) = event {
                println!("  {} acknowledged", status.action);
            }
        }

        sleep(POLL).await;
    }

    Ok(())
}
