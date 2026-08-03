use std::env;
use tastytrade::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("TastyTrade OAuth2 Authentication Example");
    info!("---------------------------------------");

    // Check if environment variables are set
    if env::var("TASTYTRADE_CLIENT_SECRET").is_err()
        || env::var("TASTYTRADE_REFRESH_TOKEN").is_err()
    {
        info!(
            "Please set TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REFRESH_TOKEN environment variables."
        );
        info!("Example:");
        info!("  export TASTYTRADE_CLIENT_SECRET=your_oauth_client_secret");
        info!("  export TASTYTRADE_REFRESH_TOKEN=your_oauth_refresh_token");
        info!("  export TASTYTRADE_USE_DEMO=true");
        info!("  export LOGLEVEL=DEBUG");
        std::process::exit(1);
    }

    // Load configuration from environment variables
    let config = TastyTradeConfig::from_env();
    info!("Configuration loaded, connecting to demo environment...");

    // Exchange the refresh token for an access token.
    let tasty = TastyTrade::connect(&config).await?;
    info!("Authenticated against {}", config.environment());

    // A duration is not a secret; a token is. This is the only thing an
    // example may print about the credential it is holding.
    match tasty.session().expires_in().await {
        Some(left) => info!("The access token is good for another {}s", left.as_secs()),
        None => info!("The access token has already expired and will be renewed on use"),
    }

    // Get account information
    let accounts = tasty.accounts().await?;
    info!("Found {} accounts:", accounts.len());

    // The split throughout: tracing gets counts and redacted identifiers,
    // because example output is routinely pasted into CI logs and support
    // threads. The operator who ran this and is looking at the terminal gets
    // the figures, on stdout.
    for account in &accounts {
        info!("Account {}", account.number().redacted());
        println!("\nAccount {}", account.number().0);

        let balance = account.balance().await?;
        println!("  Cash balance:            {}", balance.cash_balance);
        println!(
            "  Net liquidating value:   {}",
            balance.net_liquidating_value
        );
        println!(
            "  Maintenance requirement: {}",
            balance.maintenance_requirement
        );

        let positions = account.positions().await?;
        info!(
            "Account {} has {} position(s)",
            account.number().redacted(),
            positions.len()
        );
        println!("  Positions ({}):", positions.len());

        for (i, position) in positions.iter().enumerate().take(5) {
            println!(
                "    {}. {} {} {} @ {}",
                i + 1,
                position.symbol.0,
                position.quantity_direction,
                position.quantity,
                position.average_open_price
            );
        }
        if positions.len() > 5 {
            println!("    ... and {} more", positions.len() - 5);
        }

        let orders = account.live_orders().await?;
        info!(
            "Account {} has {} live order(s)",
            account.number().redacted(),
            orders.len()
        );
        println!("  Live orders ({}):", orders.len());

        for (i, order) in orders.iter().enumerate().take(3) {
            println!(
                "    {}. {} {} {} @ {}",
                i + 1,
                order.underlying_symbol.0,
                order.status,
                order.size,
                order.price
            );
        }
        if orders.len() > 3 {
            println!("    ... and {} more", orders.len() - 3);
        }
    }

    info!("OAuth2 authentication example completed successfully!");
    Ok(())
}
