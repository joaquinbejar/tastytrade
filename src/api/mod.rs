//! The REST surface of the tastytrade API.
//!
//! Every module here is a thin layer over the generic verbs in [`crate::api::client`]: it
//! names an endpoint, gives its arguments types, and returns a
//! [`crate::TastyResult`]. The decisions about status handling, redaction and
//! error shape live in [`crate::api::client`] and [`crate::api::base`], not here.

/// Accounts: balances, positions, live orders and the reviewed-placement flow.
pub mod accounts;
/// Server-side strategy backtests, on their own host.
pub mod backtesting;
/// Response envelopes, pagination and the tolerant `Items<T>` listing.
pub mod base;
/// The client itself: the OAuth2 session and the generic HTTP verbs.
pub mod client;
/// The OAuth2 session: token exchange, expiry and refresh.
pub mod oauth;

/// Option chains: flat, compact and nested.
pub mod option_chain;

/// Instruments: equities, futures, options, cryptocurrencies and warrants.
pub mod instrument;
/// Market sessions and holidays.
pub mod market_time;
/// Query-string assembly and the page a listing is asked for.
pub mod query;
/// DXLink token exchange and streamer-symbol lookup.
pub mod quote_streaming;
/// Percent-encoding for the dynamic parts of a request path.
pub(crate) mod url;
