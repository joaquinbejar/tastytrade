use std::env;
use tastytrade::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    info!("TastyTrade Demo Login Example");
    info!("-----------------------------");

    // Check if environment variables are set
    if env::var("TASTYTRADE_USERNAME").is_err() || env::var("TASTYTRADE_PASSWORD").is_err() {
        info!("Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD environment variables.");
        info!("Example:");
        info!("  export TASTYTRADE_USERNAME=your_username");
        info!("  export TASTYTRADE_PASSWORD=your_password");
        info!("  export TASTYTRADE_USE_DEMO=true");
        info!("  export LOGLEVEL=DEBUG");
        std::process::exit(1);
    }

    // Load configuration from environment variables
    let config = TastyTradeConfig::from_env();
    info!("Configuration loaded, connecting to demo environment...");

    // Login to the TastyTrade API
    let tasty = TastyTrade::login(&config).await?;
    if config.use_demo {
        info!("Successfully logged in to demo environment!");
    } else {
        info!("Successfully logged in to production environment!");
    }

    // Get account information
    let accounts = tasty.accounts().await?;
    info!("Found {} accounts:", accounts.len());

    for account in &accounts {
        info!("Account: {}", account.number().0);

        // Get account balance
        let balance = account.balance().await?;
        info!("Cash balance: {}", balance.cash_balance);
        info!("Net liquidating value: {}", balance.net_liquidating_value);
        info!(
            "Maintenance requirement: {}",
            balance.maintenance_requirement
        );

        // Get account positions
        let positions = account.positions().await?;
        info!("Positions: {}", positions.len());

        for (i, position) in positions.iter().enumerate().take(5) {
            info!(
                "  Position {}: {} - {} {} @ {}",
                i + 1,
                position.symbol.0,
                position.quantity_direction,
                position.quantity,
                position.average_open_price
            );
        }

        if positions.len() > 5 {
            info!("  ... and {} more", positions.len() - 5);
        }

        // Get live orders
        let orders = account.live_orders().await?;
        info!("Live orders: {}", orders.len());

        for (i, order) in orders.iter().enumerate().take(3) {
            info!(
                "  Order {}: {} - {} {} @ {}",
                i + 1,
                order.underlying_symbol.0,
                order.status,
                order.size,
                order.price
            );
        }

        if orders.len() > 3 {
            info!("  ... and {} more", orders.len() - 3);
        }
    }

    info!("Demo login example completed successfully!");
    Ok(())
}
