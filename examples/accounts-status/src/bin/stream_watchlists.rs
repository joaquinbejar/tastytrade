//! Subscribes to tastytrade's public watchlists and prints the changes.
//!
//! The `public-watchlists-subscribe` action. It carries no value and needs no
//! account, but it still needs an access token. Bounded by an event count and
//! a timeout, because curated lists change rarely and a silent run is the
//! normal outcome.
//!
//! ```shell
//! cargo run -p accounts-status --bin stream_watchlists
//! ```

use std::time::Duration;

use tastytrade::prelude::*;
use tokio::time::timeout;
use tracing::info;

const MAX_EVENTS: usize = 10;
const DEADLINE: Duration = Duration::from_secs(30);

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

    // The venue answers `connect-not-completed` to any other subscription
    // before a connect has landed, so subscribe to the accounts first even
    // though watchlists are not account-scoped.
    for account in &tasty.accounts().await? {
        streamer.subscribe_to_account(account).await?;
    }

    // `value` is blank for this action; the type parameter still has to be
    // named, so it is the empty vector rather than a magic string.
    streamer
        .send(
            SubRequestAction::PublicWatchlistsSubscribe,
            None::<Vec<String>>,
        )
        .await?;
    info!("Subscribed to public watchlists");

    for _ in 0..MAX_EVENTS {
        let Ok(event) = timeout(DEADLINE, streamer.get_event()).await else {
            info!("Public watchlists rarely change; stopping after {DEADLINE:?}");
            break;
        };

        match event? {
            AccountEvent::Notification(notification) => {
                if let NotificationPayload::PublicWatchlist(watchlist) = notification.payload {
                    println!(
                        "{} ({} entries)",
                        watchlist.name,
                        watchlist.watchlist_entries.len()
                    );
                    for entry in watchlist.watchlist_entries.iter().take(10) {
                        println!("  {} {:?}", entry.symbol.0, entry.instrument_type);
                    }
                }
            }
            AccountEvent::ErrorMessage(error) => {
                println!("the venue refused {}: {}", error.action, error.message);
                break;
            }
            AccountEvent::StatusMessage(status) => info!("{} acknowledged", status.action),
            AccountEvent::Unknown(unknown) => {
                info!("An unplaceable frame arrived: {:?}", unknown.kind)
            }
        }
    }

    Ok(())
}
