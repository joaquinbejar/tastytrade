
[![Dual License](https://img.shields.io/badge/license-MIT%20and%20Apache%202.0-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Downloads](https://img.shields.io/crates/d/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/stargazers)
[![Issues](https://img.shields.io/github/issues/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/issues)
[![PRs](https://img.shields.io/github/issues-pr/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/pulls)
[![Build Status](https://img.shields.io/github/workflow/status/joaquinbejar/tastytrade/CI)](https://github.com/joaquinbejar/tastytrade/actions)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/tastytrade)](https://codecov.io/gh/joaquinbejar/tastytrade)
[![Dependencies](https://img.shields.io/librariesio/github/joaquinbejar/tastytrade)](https://libraries.io/github/joaquinbejar/tastytrade)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/tastytrade)
[![Wiki](https://img.shields.io/badge/wiki-latest-blue.svg)](https://deepwiki.com/joaquinbejar/tastytrade)


## tastytrade

`tastytrade` is a Rust client library for the Tastytrade API, providing programmatic access to
trading functionality, market data, and account information.

### Features

- Authentication with Tastytrade accounts
- Real-time market data streaming via DxFeed
- Account and positions information
- Order management (placing, modifying, canceling)
- Real-time account streaming for balance updates and order status changes

### Usage

```rust
use tastytrade::TastyTrade;
use tastytrade::utils::config::TastyTradeConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Login to Tastytrade

    let config = TastyTradeConfig::from_env();
    let tasty = TastyTrade::login(&config).await?;

    // Get account information
    let accounts = tasty.accounts().await?;
    for account in accounts {
        println!("Account: {}", account.number().0);

        // Get positions
        let positions = account.positions().await?;
        println!("Positions: {}", positions.len());
    }

    Ok(())
}
```

### Real-time Data

The library supports real-time data streaming for both market data and account updates using DXLink:

```rust
// Create a quote streamer
use tastytrade::{Symbol, TastyTrade};
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::dxfeed;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TastyTradeConfig::from_env();
    let tasty = TastyTrade::login(&config)
           .await
           .unwrap();
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    let mut quote_sub = quote_streamer.create_sub(dxfeed::DXF_ET_QUOTE | dxfeed::DXF_ET_GREEKS);

    // Add symbols to subscribe to
    quote_sub.add_symbols(&[Symbol("AAPL".to_string())]);

    // Listen for events
    if let Ok(dxfeed::Event { sym, data }) = quote_sub.get_event().await {
        match data {
            dxfeed::EventData::Quote(quote) => {
                println!("Quote for {}: {}/{}", sym, quote.bid_price, quote.ask_price);
            }
            _ => {}
        }
    }
    Ok(())
}
```

 ## Setup Instructions

 1. Clone the repository:
 ```shell
 git clone https://github.com/joaquinbejar/tastytrade
 cd tastytrade
 ```

 2. Build the project:
 ```shell
 make build
 ```

 3. Run tests:
 ```shell
 make test
 ```

 4. Format the code:
 ```shell
 make fmt
 ```

 5. Run linting:
 ```shell
 make lint
 ```

 6. Clean the project:
 ```shell
 make clean
 ```

 7. Run the project:
 ```shell
 make run
 ```

 8. Fix issues:
 ```shell
 make fix
 ```

 9. Run pre-push checks:
 ```shell
 make pre-push
 ```

 10. Generate documentation:
 ```shell
 make doc
 ```

 11. Publish the package:
 ```shell
 make publish
 ```

 12. Generate coverage report:
 ```shell
 make coverage
 ```


### CLI Example

This crate also includes a sample CLI application in the `tastytrade-cli` directory
that demonstrates a portfolio viewer with real-time updates.

 ## Testing

 To run unit tests:
 ```shell
 make test
 ```

 To run tests with coverage:
 ```shell
 make coverage
 ```

 ## Contribution and Contact

 We welcome contributions to this project! If you would like to contribute, please follow these steps:

 1. Fork the repository.
 2. Create a new branch for your feature or bug fix.
 3. Make your changes and ensure that the project still builds and all tests pass.
 4. Commit your changes and push your branch to your forked repository.
 5. Submit a pull request to the main repository.

 If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

 **Joaquín Béjar García**
 - Email: jb@taunais.com
 - GitHub: [joaquinbejar](https://github.com/joaquinbejar)

 We appreciate your interest and look forward to your contributions!




## Contribution and Contact

We welcome contributions to this project! If you would like to contribute, please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

**Joaquín Béjar García**
- Email: jb@taunais.com
- GitHub: [joaquinbejar](https://github.com/joaquinbejar)

We appreciate your interest and look forward to your contributions!

## ✍️ License

Licensed under MIT license
