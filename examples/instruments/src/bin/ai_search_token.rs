//! Mints a short-lived third-party client token for AI search.
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin ai_search_token
//! ```
//!
//! Prints **that** a token was obtained and when it expires. Never the token:
//! it is a credential, `Debug` and `Display` both redact it, and the only way
//! to read it is `expose()` — which this example deliberately does not call.
//!
//! The service the token authenticates is not part of the tastytrade API this
//! crate wraps, so minting it is where this crate's job ends.

use tastytrade::prelude::*;
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::utils::logger::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();
    if !config.has_valid_credentials() {
        info!("Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN first (see .env.example)");
        return Ok(());
    }

    let tasty = TastyTrade::connect(&config).await?;
    let token = tasty.ai_search_token().await?;

    if token.is_empty() {
        info!("The venue answered with no token object.");
        return Ok(());
    }

    // Safe to print: the type renders as a byte count, not as a value.
    info!("Obtained {token}");

    match token.expires_at() {
        Some(expires_at) => info!("Expires at {expires_at}"),
        // A probe, not a contract — the published spec documents no response
        // schema for this endpoint at all.
        None => info!(
            "No expiry field this crate recognises; the response has {} key(s). \
             Read them with `field()` once the venue documents the shape.",
            token
                .expose()
                .as_object()
                .map(serde_json::Map::len)
                .unwrap_or(0)
        ),
    }

    Ok(())
}
