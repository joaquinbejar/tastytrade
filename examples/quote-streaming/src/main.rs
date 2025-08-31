use tastytrade::Symbol;
use tastytrade::TastyTrade;
use tastytrade::dxfeed::{self, EventData};
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

    let mut streamer = match tasty.create_quote_streamer().await {
        Ok(s) => {
            println!("✅ Quote streamer created successfully!");
            s
        }
        Err(e) => {
            eprintln!("❌ Failed to create quote streamer: {}", e);
            std::process::exit(1);
        }
    };

    // Create a subscription for SPX quotes
    let mut quote_sub = streamer.create_sub(dxfeed::DXF_ET_QUOTE);

    // Subscribe to SPX symbol
    let symbols = [Symbol::from("SPX")];
    quote_sub.add_symbols(&symbols);

    println!("📈 Streaming quotes for SPX...");
    println!("Press Ctrl+C to stop\n");

    // Stream quote events
    loop {
        match quote_sub.get_event().await {
            Ok(ev) => {
                if let EventData::Quote(data) = ev.data {
                    println!(
                        "{}: Bid: ${:.2} / Ask: ${:.2}",
                        ev.sym, data.bid_price, data.ask_price
                    );
                }
            }
            Err(e) => {
                eprintln!("❌ Error receiving quote: {:?}", e);
                break;
            }
        }
    }
}
