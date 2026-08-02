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

    info!("Demo login example completed successfully!");
    Ok(())
}
