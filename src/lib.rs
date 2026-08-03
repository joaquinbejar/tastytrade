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
//! A Rust client for the tastytrade brokerage API. **Orders placed through it
//! move real money.**
//!
//! This is the API reference. The [README] is the tour: what the crate covers,
//! how to authenticate, and a worked example per area. What follows is the
//! handful of behaviours a caller has to know before reading any individual
//! method, because they are properties of the whole crate rather than of one
//! call.
//!
//! [README]: https://github.com/joaquinbejar/tastytrade#readme
//!
//! ## Certification is the default
//!
//! [`utils::config::TastyTradeConfig::from_env`] selects the **certification**
//! environment (`api.cert.tastyworks.com`). Production is a deliberate opt-in:
//!
//! ```shell
//! TASTYTRADE_USE_DEMO=false   # production — orders placed here are real
//! ```
//!
//! Only a value that parses as `false` selects production. A missing, empty or
//! misspelled variable resolves to certification, so a typo cannot be what
//! points an order at a funded account.
//!
//! A session is bound to the deployment it authenticated against: it will not
//! present a certification token to production, and it will not send the client
//! secret to a host it did not authenticate with.
//!
//! ## Authentication is OAuth2, and only OAuth2
//!
//! tastytrade **decommissioned `POST /sessions` on 2026-02-11**. Username and
//! password authentication, session tokens and remember tokens are gone from
//! the venue and gone from this crate with it.
//!
//! ```rust,no_run
//! use tastytrade::TastyTrade;
//! use tastytrade::utils::config::TastyTradeConfig;
//!
//! # async fn connect() -> Result<(), Box<dyn std::error::Error>> {
//! let config = TastyTradeConfig::from_env();
//! let tasty = TastyTrade::connect(&config).await?;
//!
//! for account in tasty.accounts().await? {
//!     // Redacted: doc examples get copied, and an account number in a log is
//!     // the thing this crate spends most of its care avoiding.
//!     println!("{}", account.number().redacted());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Access tokens last about fifteen minutes and every request renews the one in
//! hand before it expires, so a long-lived client keeps working. A renewal is
//! never a *retry*: a `POST` that may have placed an order is not replayed on a
//! `401`.
//!
//! [`TastyTrade::connect_with_authorization_code`] is the third-party grant, for
//! an application acting on somebody else's account.
//!
//! ## Nothing that trades happens without a receipt
//!
//! Placement, replacement, editing and complex orders all go through the same
//! shape: dry-run, read what the venue said, then apply the receipt.
//!
//! ```rust,no_run
//! # use tastytrade::prelude::*;
//! # async fn place(account: &Account<'_>, order: &Order)
//! # -> Result<(), Box<dyn std::error::Error>> {
//! let receipt = account.review_order(order).await?;
//!
//! for warning in receipt.warnings() {
//!     // Venue prose, written for a person: it can name the account or the
//!     // buying power, so it belongs on a screen rather than in a log.
//!     println!("{warning}");
//! }
//!
//! // `accept` refuses when there are warnings. That is not a refusal to
//! // proceed — it is a refusal to proceed *silently*.
//! let reviewed = receipt.accept()?;
//! account.place_reviewed_order(reviewed).await?;
//! # Ok(())
//! # }
//! ```
//!
//! A receipt binds the account number **and** the deployment, because
//! certification reuses production account numbering — without the origin, a
//! sandbox dry run would authorise a real order against the same number. No
//! receipt is `Clone`: duplicable proof is not proof.
//!
//! [`accounts::Account::place_order`] still exists for callers managing the
//! review themselves. It carries no evidence that one happened.
//!
//! ## Money is `Decimal`
//!
//! Every price, quantity, balance and ratio is [`rust_decimal::Decimal`].
//! `f64` appears in exactly one place — [`types::dxfeed`], where the streaming
//! feed imposes it — and REST paths never reuse those types even when the field
//! names match.
//!
//! ## An absent field is unknown, never zero
//!
//! A flag the venue did not send is `None`, not `false`; a price it did not send
//! is `None`, not `0`. Certification omits fields production sends, and
//! "we were not told whether this account is frozen" and "this account is not
//! frozen" are different facts — only one of them is safe to act on.
//!
//! ## Secrets never render themselves
//!
//! The client secret, the refresh and access tokens, the DXLink quote token, the
//! AI-search token and the whole customer resource print as `***` or as a field
//! count. Not in `Debug`, not in `Display`, not in a log line, not in an error
//! message — an error is a string the caller prints wherever they like.
//!
//! Account numbers are redacted from every request path that reaches an error,
//! and a response body is never logged at any level: an error document from an
//! endpoint this crate does not control can echo a credential.
//!
//! ## A library does not panic
//!
//! No `unwrap`, no `expect`, no unchecked indexing on any path reachable from a
//! public method. Everything fallible returns [`TastyTradeError`]. A local
//! failure is [`TastyTradeError::Precondition`] and reports `is_retryable()`
//! false, because nothing was sent.
//!
//! ## Unknown values survive
//!
//! [`api::base::Items`] skips an item it cannot decode rather than failing a
//! whole listing, so a strict enum on a response would make a row **disappear**
//! — silently. The response enums therefore keep an `Unknown(String)` arm that
//! round-trips the venue's text: a new order status, transaction kind or
//! instrument classification is visible and matchable instead of missing.
//!
//! Request enums are closed, for the opposite reason: tolerance there would only
//! let a caller send something the venue rejects.
//!
//! ## Cryptocurrency order routing is suspended
//!
//! tastytrade disabled it on 2026-06-29, until further notice. An order with a
//! cryptocurrency leg is refused locally on every routing path. **Instrument
//! discovery and market data are unaffected.** The whole decision is
//! [`prelude::CRYPTOCURRENCY_TRADING_ENABLED`], one constant.
//!
//! ## Streaming
//!
//! Two websockets, and they are different services. Market data is DXLink,
//! reached with a token from `GET /api-quote-tokens`; account notifications are
//! tastytrade's own streamer, authenticated with the access token. Both
//! reconnect under a [`streaming::reconnect::BackoffPolicy`] and expose
//! [`streaming::reconnect::ConnectionState`].
//!
//! Candles are the only route to a price series in this crate, and the only
//! subscription needing more than a symbol — a candle is addressed by a symbol
//! carrying its period, `AAPL{=5m}`.
//!
//! ```rust,no_run
//! # use chrono::{Duration, Utc};
//! # use tastytrade::{Symbol, TastyTrade};
//! # use tastytrade::dxfeed::{CandlePeriod, EventData, EventKind};
//! # async fn bars(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
//! let mut streamer = tasty.create_quote_streamer().await?;
//! let mut bars = streamer.create_sub([EventKind::Candle]).await?;
//!
//! // `from_time` is required, not optional: without one a candle subscription
//! // replays an unbounded history.
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
//!     println!("{}: o {} c {}", event.sym, candle.open, candle.close);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! A subscription's buffer is bounded, so a slow consumer loses events rather
//! than stalling every other subscription.
//! [`streaming::quote_streamer::QuoteSubscription::lagged`] makes that
//! observable, and for candles it is recoverable across a reconnect: a dropped
//! bar stops the resume point advancing, so the next connection asks for it
//! again.
//!
//! The account websocket publishes a **full object** on every change — never a
//! diff. The fills inside an order's legs are the only place an executed price
//! reaches this crate; no REST endpoint returns one. Anything that is JSON
//! reaches the caller, including a `type` nobody here recognises.
//!
//! ## Where to look
//!
//! [`prelude`] re-exports the advertised surface in one import. The endpoint
//! groups hang off [`TastyTrade`] and [`accounts::Account`]; the filters that
//! narrow them are `*Filter` types taking a [`api::query::PageRequest`].

/// Compiles every Rust block in `README.md` as a doc test.
///
/// The README is hand-written now, which means nothing else checks its
/// examples — and a README whose code does not compile is worse than one with
/// no code at all. This couples the *code* to the crate without coupling the
/// *prose*: the file is still authored by hand, and `src/lib.rs` no longer
/// generates it.
///
/// `#[cfg(doctest)]` keeps the item out of every build except the doc-test
/// pass, so it costs nothing at compile time and appears in no documentation.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;

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
