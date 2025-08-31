use tastytrade::TastyTrade;
use tastytrade::utils::config::Config;

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    // Check if credentials are configured
    if !config.has_valid_credentials() {
        eprintln!("Error: Missing TastyTrade credentials!");
        eprintln!("Please make sure you have:");
        eprintln!("1. Copied .env.example to .env: cp .env.example .env");
        eprintln!("2. Set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD in .env");
        eprintln!("3. Set TASTYTRADE_USE_DEMO=true for sandbox testing");
        std::process::exit(1);
    }

    println!("Attempting to login with username: {}", config.username);
    println!("Using demo environment: {}", config.use_demo);
    println!("Base URL: {}", config.base_url);

    let tasty = match TastyTrade::login(&config).await {
        Ok(client) => {
            println!("✅ Login successful!");
            client
        }
        Err(e) => {
            eprintln!("❌ Login failed: {}", e);
            eprintln!("\nTroubleshooting:");
            eprintln!("1. Verify your credentials are correct");
            eprintln!("2. Make sure TASTYTRADE_USE_DEMO=true for sandbox");
            eprintln!("3. Check if your account has API access enabled");
            std::process::exit(1);
        }
    };

    let accounts = match tasty.accounts().await {
        Ok(accounts) => {
            println!("✅ Retrieved {} account(s)", accounts.len());
            accounts
        }
        Err(e) => {
            eprintln!("❌ Failed to get accounts: {}", e);
            std::process::exit(1);
        }
    };

    for account in accounts {
        println!("\n📊 Account: {}", account.number().0);

        match account.positions().await {
            Ok(positions) => {
                let symbols: Vec<String> = positions.into_iter().map(|p| p.symbol.0).collect();
                println!("   Positions ({}): {:?}", symbols.len(), symbols);
            }
            Err(e) => {
                eprintln!(
                    "   ❌ Failed to get positions for account {}: {}",
                    account.number().0,
                    e
                );
            }
        }
    }
}
