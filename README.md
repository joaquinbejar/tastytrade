
[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Downloads](https://img.shields.io/crates/d/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/stargazers)
[![Issues](https://img.shields.io/github/issues/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/issues)
[![PRs](https://img.shields.io/github/issues-pr/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/pulls)
[![Tests](https://img.shields.io/github/actions/workflow/status/joaquinbejar/tastytrade/tests.yml?branch=main&label=tests)](https://github.com/joaquinbejar/tastytrade/actions/workflows/tests.yml)
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
        // Redacted: doc examples get copied, and an account number in a
        // log is the thing this crate spends most of its care avoiding.
        println!("Account: {}", account.number().redacted());

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
    let tasty = TastyTrade::login(&config).await?;
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    let mut quote_sub = quote_streamer.create_sub(dxfeed::DXF_ET_QUOTE | dxfeed::DXF_ET_GREEKS);

    // Add symbols to subscribe to
    quote_sub.add_symbols(&[Symbol("AAPL".to_string())]).await?;

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

### Environments

`TastyTradeConfig::from_env` selects the **certification** environment by
default (`api.cert.tastyworks.com`). Production is a deliberate opt-in:

```shell
TASTYTRADE_USE_DEMO=false   # production — orders placed here are real
```

Only a value that parses as `false` selects production. A missing, empty or
misspelled variable resolves to certification, so a typo cannot be what
points an order at a funded account.

Logging in without `TASTYTRADE_USERNAME` and `TASTYTRADE_PASSWORD` fails
locally with `TastyTradeError::ConfigError` and never reaches the network.

### Placing an order

Placement goes through a review the venue's warnings cannot be skipped
past silently:

```rust
let receipt = account.review_order(order).await?;
println!("buying power effect: {}", receipt.result().buying_power_effect.change_in_buying_power);

if !receipt.is_clean() {
    for warning in receipt.warnings() {
        println!("warning: {}", warning.message);
    }
    // accept_with_warnings is for a person who has read the above and
    // still wants the order. Reaching for it automatically defeats it.
    return Ok(());
}
let reviewed = receipt.accept()?;

// Certification only. An example that places on production is an example
// somebody runs on production.
if config.use_demo {
    account.place_reviewed_order(reviewed).await?;
}
```

`Account::place_order` still exists for callers that manage the review
themselves, but it carries no evidence that a review happened.

### Logging

This crate emits `tracing` events and does **not** install a subscriber on
your behalf: process-global logging belongs to the application. Loading
configuration touches no global state, so a program that already owns
`tracing` keeps its own setup.

Binaries that do not want to build one can opt in:

```rust
use tastytrade::utils::logger::{try_setup_logger, LoggerInit};

match try_setup_logger() {
    LoggerInit::Installed => {}
    LoggerInit::AlreadyInstalled => { /* the application owns it */ }
    LoggerInit::Unsupported => { /* wasm32 */ }
}
```

Without a subscriber the environment warnings above are not printed, so a
binary that cares about them should install one before building the config.

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
