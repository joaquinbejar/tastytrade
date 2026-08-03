//! Builds the URL a trusted third-party application sends a customer to.
//!
//! Prints and exits: there is nothing to connect to until a real customer has
//! authorized, and the code that comes back is single-use. The rest of that
//! flow — verifying `state`, exchanging the code — is in the crate docs.
//!
//! ```shell
//! export TASTYTRADE_CLIENT_ID=your_oauth_client_id
//! export TASTYTRADE_REDIRECT_URI=https://your-app.example.com/oauth/callback
//! cargo run -p miscellaneous --bin authorize_url
//! ```

use tastytrade::prelude::*;
use tracing::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let config = TastyTradeConfig::from_env();

    if config.client_id.trim().is_empty() || config.redirect_uri.trim().is_empty() {
        info!("Set TASTYTRADE_CLIENT_ID and TASTYTRADE_REDIRECT_URI first.");
        info!("Both come from the OAuth application you registered with tastytrade;");
        info!("the redirect URI has to match one registered there exactly.");
        return Ok(());
    }

    let request = AuthorizationRequest::new(&config.client_id, &config.redirect_uri)
        .with_scopes([Scope::Read, Scope::Trade])
        // A real application generates this per browser session and stores it,
        // so the redirect can be tied back to the request that started it.
        // A constant here would prove nothing, which is why it says so.
        .with_state("example-state-replace-me");

    let url = request.authorize_url(config.environment())?;

    info!("Authorizing against {}", config.environment());
    // stdout, so it can be piped into a browser. It carries the client id and
    // the redirect URI, both public; the client secret belongs in the token
    // request and never in a URL a browser keeps in its history.
    println!("{url}");

    Ok(())
}
