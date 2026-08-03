// The public API is the crate contract, and an undocumented item is a
// contract nobody can read. Denied at the crate root rather than in a
// Makefile target, so it fails at the same moment as any other compile
// error and cannot be forgotten by running a different command.
#![deny(missing_docs)]
// A doc link that points nowhere is documentation that lies about where to
// look next, which is worse than not linking at all.
#![deny(rustdoc::broken_intra_doc_links)]

//! # tastytrade
//!
//! `tastytrade` is a Rust client library for the Tastytrade API, providing programmatic access to
//! trading functionality, market data, and account information.
//!
//! ## Features
//!
//! - OAuth2 authentication, with automatic access-token renewal
//! - Real-time market data streaming via DxFeed
//! - Account and positions information
//! - Order management (placing, modifying, canceling)
//! - Real-time account streaming for balance updates and order status changes
//!
//! ## Authentication
//!
//! tastytrade **decommissioned `POST /sessions` on 2026-02-11**. Username and
//! password authentication, session tokens and remember tokens are gone from
//! the venue, and gone from this crate with it; OAuth2 is the only flow that
//! works.
//!
//! Create an OAuth application and a personal grant under Manage → My Profile
//! → API on [my.tastytrade.com](https://my.tastytrade.com). That gives you a
//! **client secret** and a **refresh token**, which are what
//! [`utils::config::TastyTradeConfig`] reads from
//! `TASTYTRADE_CLIENT_SECRET` and `TASTYTRADE_REFRESH_TOKEN`.
//!
//! Access tokens last about fifteen minutes. You do not have to manage that:
//! every request renews the token first when the one in hand is about to
//! expire, so a long-lived client keeps working. A renewal is never a *retry* —
//! a `POST` that may have placed an order is not replayed on a `401`.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use tastytrade::TastyTrade;
//! use tastytrade::utils::config::TastyTradeConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Authenticate with the personal refresh-token grant
//!
//!     let config = TastyTradeConfig::from_env();
//!     let tasty = TastyTrade::connect(&config).await?;
//!
//!     // Get account information
//!     let accounts = tasty.accounts().await?;
//!     for account in accounts {
//!         // Redacted: doc examples get copied, and an account number in a
//!         // log is the thing this crate spends most of its care avoiding.
//!         println!("Account: {}", account.number().redacted());
//!         
//!         // Get positions
//!         let positions = account.positions().await?;
//!         println!("Positions: {}", positions.len());
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Real-time Data
//!
//! Market data comes over DXLink. All eleven event types the feed models are
//! routed: quotes, regular and extended-hours trade prints, Greeks, candles,
//! summaries, time and sale, profiles, underlyings, theoretical prices and
//! series. A subscription names the ones it wants and the channel is
//! configured for exactly those.
//!
//! ```rust,no_run
//! // Create a quote streamer
//! use tastytrade::{Symbol, TastyTrade};
//! use tastytrade::utils::config::TastyTradeConfig;
//! use tastytrade::dxfeed::{self, EventKind};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = TastyTradeConfig::from_env();
//!     let tasty = TastyTrade::connect(&config).await?;
//!     let mut quote_streamer = tasty.create_quote_streamer().await?;
//!     let mut quote_sub = quote_streamer.create_sub([EventKind::Quote, EventKind::Greeks]);
//!
//!     // Add symbols to subscribe to
//!     quote_sub.add_symbols(&[Symbol("AAPL".to_string())]).await?;
//!
//!     // Listen for events
//!     if let Ok(dxfeed::Event { sym, data }) = quote_sub.get_event().await {
//!         match data {
//!             dxfeed::EventData::Quote(quote) => {
//!                 println!("Quote for {}: {}/{}", sym, quote.bid_price, quote.ask_price);
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Candles
//!
//! Candles are the only route to a price series in this crate — there is no
//! REST endpoint for one — and the only subscription that needs more than a
//! symbol. A candle is addressed by a symbol that carries its period,
//! `AAPL{=5m}`, so two periods of one underlying are two different streamer
//! symbols and never deliver into each other.
//!
//! ```rust,no_run
//! use chrono::{Duration, Utc};
//! use tastytrade::{Symbol, TastyTrade};
//! use tastytrade::dxfeed::{CandlePeriod, EventData, EventKind};
//!
//! # async fn bars(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
//! let mut streamer = tasty.create_quote_streamer().await?;
//! let mut bars = streamer.create_sub([EventKind::Candle]);
//!
//! // `from_time` is required, not optional: a candle subscription without one
//! // replays an unbounded history. A day of one-minute bars is about 1440
//! // events per symbol.
//! bars.add_candles(
//!     &[Symbol("AAPL".to_string())],
//!     CandlePeriod::minutes(5)?,
//!     Utc::now() - Duration::days(2),
//! )
//! .await?;
//!
//! if let Ok(event) = bars.get_event().await
//!     && let EventData::Candle(candle) = event.data
//! {
//!     // `event.sym` is `AAPL{=5m}`, period included.
//!     println!("{}: o {} c {}", event.sym, candle.open, candle.close);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! A reconnect resumes each candle series one millisecond past the last bar
//! actually delivered, rather than replaying the original start — otherwise
//! every reconnect would re-send a history the consumer already has.
//!
//! ## Account streaming
//!
//! The account websocket publishes a **full object** on every change — never a
//! diff — for orders, balances, positions, quote alerts and tastytrade's
//! public watchlists. The fills inside an order's `legs` are the only place an
//! executed price reaches this crate: no REST endpoint returns one.
//!
//! ```rust,no_run
//! use tastytrade::prelude::*;
//!
//! # async fn watch(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
//! let streamer = tasty.create_account_streamer().await?;
//! for account in &tasty.accounts().await? {
//!     streamer.subscribe_to_account(account).await?;
//! }
//!
//! match streamer.get_event().await? {
//!     AccountEvent::Notification(notification) => match notification.payload {
//!         NotificationPayload::Order(order) => {
//!             for leg in &order.legs {
//!                 for fill in &leg.fills {
//!                     println!("{:?} at {:?}", fill.quantity, fill.fill_price);
//!                 }
//!             }
//!         }
//!         // A notification type this crate does not model yet still arrives,
//!         // with its payload. Nothing is discarded.
//!         NotificationPayload::Unsupported(payload) => {
//!             println!("{} arrived untyped ({} bytes)", notification.kind, payload.len());
//!         }
//!         _ => {}
//!     },
//!     AccountEvent::Unknown(unknown) => println!("unplaceable: {:?}", unknown.kind),
//!     _ => {}
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Anything that is JSON reaches the caller. A `type` nobody here recognises,
//! a payload that does not match its model, and a frame that is neither a
//! notification nor an acknowledgement all arrive with the payload intact —
//! `RawPayload` renders as a byte count, so reading it takes
//! [`RawPayload::expose`](prelude::RawPayload::expose) and is one grep away
//! from an audit. Only bytes that are not JSON are dropped, and that is
//! reported without the frame or the serde error.
//!
//! ## Environments
//!
//! `TastyTradeConfig::from_env` selects the **certification** environment by
//! default (`api.cert.tastyworks.com`). Production is a deliberate opt-in:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false   # production — orders placed here are real
//! ```
//!
//! Only a value that parses as `false` selects production. A missing, empty or
//! misspelled variable resolves to certification, so a typo cannot be what
//! points an order at a funded account.
//!
//! Connecting without `TASTYTRADE_CLIENT_SECRET` and
//! `TASTYTRADE_REFRESH_TOKEN` fails locally with
//! `TastyTradeError::ConfigError` and never reaches the network.
//!
//! A session is bound to the deployment it authenticated against: it will not
//! present a certification token to production, and it will not send the
//! client secret to a host it did not authenticate with.
//!
//! ## Authorizing other people's accounts
//!
//! A **trusted third-party** application — one tastytrade has reviewed — sends
//! a customer to the authorization page and exchanges the code it gets back:
//!
//! ```rust,no_run
//! use tastytrade::TastyTrade;
//! use tastytrade::oauth::{AuthorizationRequest, Scope};
//! use tastytrade::utils::config::TastyTradeConfig;
//!
//! # async fn authorize(state: &str, code: String, returned_state: Option<&str>)
//! # -> Result<(), Box<dyn std::error::Error>> {
//! let config = TastyTradeConfig::from_env();
//!
//! let request = AuthorizationRequest::new(&config.client_id, &config.redirect_uri)
//!     .with_scopes([Scope::Read, Scope::Trade])
//!     // Tie this to the browser session that started the flow. This crate
//!     // does not invent one: a nonce the application cannot correlate
//!     // proves nothing.
//!     .with_state(state);
//!
//! // Send the customer here. The URL carries no secret.
//! let url = request.authorize_url(config.environment())?;
//!
//! // …they come back to your redirect URI with `code` and `state`.
//! request.verify_state(returned_state)?;
//! let tasty = TastyTrade::connect_with_authorization_code(&config, code).await?;
//!
//! // Store this. It does not expire, and having it means never sending the
//! // customer through the authorization page again.
//! let refresh_token = tasty.refresh_token().await;
//! # let _ = (url, refresh_token);
//! # Ok(())
//! # }
//! ```
//!
//! ## Placing an order
//!
//! Placement goes through a review the venue's warnings cannot be skipped
//! past silently:
//!
//! ```rust,no_run
//! # use tastytrade::{Order, TastyTrade};
//! # use tastytrade::utils::config::TastyTradeConfig;
//! # async fn place(
//! #     account: &tastytrade::accounts::Account<'_>,
//! #     order: &Order,
//! #     config: &TastyTradeConfig,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
//! let receipt = account.review_order(order).await?;
//! println!("buying power effect: {}", receipt.result().buying_power_effect.change_in_buying_power);
//!
//! if !receipt.is_clean() {
//!     for warning in receipt.warnings() {
//!         println!("warning: {}", warning.message);
//!     }
//!     // accept_with_warnings is for a person who has read the above and
//!     // still wants the order. Reaching for it automatically defeats it.
//!     return Ok(());
//! }
//! let reviewed = receipt.accept()?;
//!
//! // Certification only. An example that places on production is an example
//! // somebody runs on production.
//! if config.use_demo {
//!     account.place_reviewed_order(reviewed).await?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! `Account::place_order` still exists for callers that manage the review
//! themselves, but it carries no evidence that a review happened.
//!
//! ## Logging
//!
//! This crate emits `tracing` events and does **not** install a subscriber on
//! your behalf: process-global logging belongs to the application. Loading
//! configuration touches no global state, so a program that already owns
//! `tracing` keeps its own setup.
//!
//! Binaries that do not want to build one can opt in:
//!
//! ```rust,no_run
//! use tastytrade::utils::logger::{try_setup_logger, LoggerInit};
//!
//! match try_setup_logger() {
//!     LoggerInit::Installed => {}
//!     LoggerInit::AlreadyInstalled => { /* the application owns it */ }
//!     LoggerInit::Unsupported => { /* wasm32 */ }
//! }
//! ```
//!
//! Without a subscriber the environment warnings above are not printed, so a
//! binary that cares about them should install one before building the config.
//!
//!  ## Setup Instructions
//!  
//!  1. Clone the repository:
//!  ```shell
//!  git clone https://github.com/joaquinbejar/tastytrade
//!  cd tastytrade
//!  ```
//!  
//!  2. Build the project:
//!  ```shell
//!  make build
//!  ```
//!  
//!  3. Run tests:
//!  ```shell
//!  make test
//!  ```
//!  
//!  4. Format the code:
//!  ```shell
//!  make fmt
//!  ```
//!  
//!  5. Run linting:
//!  ```shell
//!  make lint
//!  ```
//!  
//!  6. Clean the project:
//!  ```shell
//!  make clean
//!  ```
//!  
//!  7. Run the project:
//!  ```shell
//!  make run
//!  ```
//!  
//!  8. Fix issues:
//!  ```shell
//!  make fix
//!  ```
//!  
//!  9. Run pre-push checks:
//!  ```shell
//!  make pre-push
//!  ```
//!  
//!  10. Generate documentation:
//!  ```shell
//!  make doc
//!  ```
//!  
//!  11. Publish the package:
//!  ```shell
//!  make publish
//!  ```
//!  
//!  12. Generate coverage report:
//!  ```shell
//!  make coverage
//!  ```
//!
//!
//! ## CLI Example
//!
//! This crate also includes a sample CLI application in the `tastytrade-cli` directory
//! that demonstrates a portfolio viewer with real-time updates.
//!
//! It reads its credentials from the environment, and takes `--config` to read
//! them from a JSON file instead. Neither credential is a flag on purpose: a
//! secret given on the command line is visible to every process on the machine
//! and is kept in the shell history.
//!
//!  ```shell
//!  export TASTYTRADE_CLIENT_SECRET=...
//!  export TASTYTRADE_REFRESH_TOKEN=...
//!  export TASTYTRADE_USE_DEMO=true      # certification, the safe default
//!  cargo run -p tastytrade-cli
//!  ```
//!
//! ## Migrating from 0.3
//!
//! Every authentication surface changed, because the API behind it was
//! retired. This is a breaking release and `cargo semver-checks` reports it as
//! one.
//!
//! | Removed | Replacement |
//! |---|---|
//! | `TastyTrade::login(&config)` | [`TastyTrade::connect`] |
//! | `TastyTrade::default()` | [`TastyTrade::from_env`] |
//! | `LoginCredentials`, `LoginResponse`, `LoginResponseUser` | [`oauth::TokenResponse`] |
//! | `TastyTradeConfig::username`, `::password` | `client_secret`, `refresh_token` |
//! | `TastyTradeConfig::remember_me`, `TASTYTRADE_REMEMBER_ME` | nothing — it configured a retired API |
//! | `TASTYTRADE_USERNAME`, `TASTYTRADE_PASSWORD` | `TASTYTRADE_CLIENT_SECRET`, `TASTYTRADE_REFRESH_TOKEN` |
//! | CLI `--login` | CLI `--config` |
//!
//! There is no deprecation window: a deprecated `login()` would still be a
//! call to an endpoint that no longer exists, so leaving one in place would
//! only move the failure from compile time to run time.
//!  
//!  ## Testing
//!  
//!  To run unit tests:
//!  ```shell
//!  make test
//!  ```
//!  
//!  To run tests with coverage:
//!  ```shell
//!  make coverage
//!  ```
//!  
//!  ## Contribution and Contact
//!  
//!  We welcome contributions to this project! If you would like to contribute, please follow these steps:
//!  
//!  1. Fork the repository.
//!  2. Create a new branch for your feature or bug fix.
//!  3. Make your changes and ensure that the project still builds and all tests pass.
//!  4. Commit your changes and push your branch to your forked repository.
//!  5. Submit a pull request to the main repository.
//!  
//!  If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:
//!  
//!  **Joaquín Béjar García**
//!  - Email: jb@taunais.com
//!  - GitHub: [joaquinbejar](https://github.com/joaquinbejar)
//!  
//!  We appreciate your interest and look forward to your contributions!
//!  

/// REST surface: authentication, accounts, instruments and option chains.
pub mod api;
mod error;
/// Real-time transports: DXLink quotes and the account websocket.
pub mod streaming;
mod types;

/// The commonly used types in one import.
pub mod prelude;
/// Configuration, logging, bulk downloads and parsing helpers.
pub mod utils;

pub use api::accounts;
pub use api::base::TastyResult;
pub use api::client::TastyTrade;

pub use error::{ApiError, DxFeedError, Environment, RequestContext, TastyTradeError};
pub use types::dxfeed;
pub use types::instrument::InstrumentType;
pub use types::oauth;
pub use types::order::{
    Action, Order, OrderBuilder, OrderLeg, OrderLegBuilder, OrderType, PriceEffect, TimeInForce,
};
pub use types::order::{AsSymbol, LiveOrderRecord, Symbol};
pub use types::position::{BriefPosition, FullPosition, QuantityDirection};
pub use types::wire::{ExerciseStyle, ExpirationType, SettlementType};
