//! Subscribes to account updates and prints what arrives.
//!
//! The `connect` action, which publishes orders, balances and positions. Runs
//! against certification with no arguments and stops after a bounded number of
//! events or a timeout, so it can be checked rather than merely started.
//!
//! ```shell
//! cargo run -p accounts-status --bin stream_account
//! ```

use std::time::Duration;

use tastytrade::prelude::*;
use tokio::time::timeout;
use tracing::info;

/// Enough to see the shape of the stream without waiting on a quiet market.
const MAX_EVENTS: usize = 20;
/// A quiet account is the normal case out of hours, so this has to end anyway.
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
    let accounts = tasty.accounts().await?;
    info!("Authenticated against {}", config.environment());

    let streamer = tasty.create_account_streamer().await?;
    for account in &accounts {
        streamer.subscribe_to_account(account).await?;
        // Redacted: example output ends up in issues and CI logs.
        info!("Subscribed to account {}", account.number().redacted());
    }

    info!("Listening for up to {MAX_EVENTS} events or {DEADLINE:?}");

    for _ in 0..MAX_EVENTS {
        let Ok(event) = timeout(DEADLINE, streamer.get_event()).await else {
            info!("No further events within {DEADLINE:?}; stopping");
            break;
        };

        match event? {
            AccountEvent::Notification(notification) => {
                // stdout for the operator at the terminal, who owns this data.
                println!("[{}] {:?}", notification.kind, notification.timestamp);
                match notification.payload {
                    NotificationPayload::Order(order) => {
                        println!("  order {} is {}", order.id.0, order.status);
                        for leg in &order.legs {
                            for fill in &leg.fills {
                                println!(
                                    "  filled {:?} of {} at {:?}",
                                    fill.quantity, leg.symbol.0, fill.fill_price
                                );
                            }
                        }
                    }
                    NotificationPayload::AccountBalance(balance) => {
                        println!("  net liquidating value {}", balance.net_liquidating_value);
                    }
                    NotificationPayload::CurrentPosition(position) => {
                        println!(
                            "  {} {} {}",
                            position.symbol.0, position.quantity_direction, position.quantity
                        );
                    }
                    other => println!("  {other:?}"),
                }
            }
            AccountEvent::StatusMessage(status) => {
                info!("{} acknowledged", status.action);
            }
            AccountEvent::ErrorMessage(error) => {
                // Venue prose: in front of a person, never into a log line.
                println!("the venue refused {}: {}", error.action, error.message);
            }
            AccountEvent::Unknown(unknown) => {
                info!("An unplaceable frame arrived: {:?}", unknown.kind);
            }
        }
    }

    Ok(())
}
