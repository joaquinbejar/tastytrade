use tastytrade::prelude::*;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() {
    setup_logger();
    let config = TastyTradeConfig::new();

    // Check if credentials are configured
    if !config.has_valid_credentials() {
        error!("Error: Missing TastyTrade credentials!");
        error!("Please make sure you have:");
        error!("1. Copied .env.example to .env: cp .env.example .env");
        error!("2. Set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN in .env");
        error!("3. Set TASTYTRADE_USE_DEMO=true for sandbox testing");
        std::process::exit(1);
    }

    debug!("Authenticating against {}", config.environment());
    debug!("Using demo environment: {}", config.use_demo);
    debug!("Base URL: {}", config.base_url);

    let tasty = match TastyTrade::connect(&config).await {
        Ok(client) => {
            info!("✅ Login successful!");
            client
        }
        Err(e) => {
            error!("❌ Login failed: {}", e);
            error!("\nTroubleshooting:");
            error!("1. Verify your credentials are correct");
            error!("2. Make sure TASTYTRADE_USE_DEMO=true for sandbox");
            error!("3. Check if your account has API access enabled");
            std::process::exit(1);
        }
    };

    let accounts = match tasty.accounts().await {
        Ok(accounts) => {
            debug!("✅ Retrieved {} account(s)", accounts.len());
            accounts
        }
        Err(e) => {
            error!("❌ Failed to get accounts: {}", e);
            std::process::exit(1);
        }
    };

    for account in accounts {
        info!("Account {}", account.number().redacted());
        println!("\nAccount {}", account.number().0);

        match account.positions().await {
            Ok(positions) => {
                let symbols: Vec<String> = positions.into_iter().map(|p| p.symbol.0).collect();
                info!(
                    "Account {} has {} position(s)",
                    account.number().redacted(),
                    symbols.len()
                );
                println!("   Positions ({}): {:?}", symbols.len(), symbols);
            }
            Err(e) => {
                error!(
                    "   ❌ Failed to get positions for account {}: {}",
                    account.number().0,
                    e
                );
            }
        }
    }
}
