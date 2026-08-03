//! Subscribes to quote alerts and prints them as they fire.
//!
//! The `quote-alerts-subscribe` action. Alerts exist at the **user** level
//! rather than the account level, which is why nothing printed here names an
//! account. Bounded: an alert only fires when its threshold is crossed, so a
//! run that prints nothing is the normal outcome and has to end anyway.
//!
//! Create the alerts themselves with `POST /quote-alerts`, which this crate
//! does not implement yet (#81).
//!
//! ```shell
//! cargo run -p accounts-status --bin stream_quote_alerts
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

    // `connect` first: the venue answers `connect-not-completed` to any other
    // subscription that arrives before one.
    for account in &tasty.accounts().await? {
        streamer.subscribe_to_account(account).await?;
    }

    streamer
        .send(SubRequestAction::QuoteAlertsSubscribe, None::<Vec<String>>)
        .await?;
    info!("Subscribed to quote alerts; they fire only when a threshold is crossed");

    for _ in 0..MAX_EVENTS {
        let Ok(event) = timeout(DEADLINE, streamer.get_event()).await else {
            info!("No alert fired within {DEADLINE:?}; stopping");
            break;
        };

        match event? {
            AccountEvent::Notification(notification) => {
                if let NotificationPayload::QuoteAlert(alert) = notification.payload {
                    println!(
                        "{:?} {:?} {:?} (threshold {:?})",
                        alert.symbol.map(|symbol| symbol.0),
                        alert.field,
                        alert.operator,
                        alert.threshold_numeric
                    );
                    println!("  triggered at {:?}", alert.triggered_at);
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
