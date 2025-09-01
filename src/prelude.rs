/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/

//! # Prelude
//!
//! This module provides a convenient way to import the most commonly used types and traits
//! from the tastytrade library. By importing this prelude, you get access to all the essential
//! components needed for most tastytrade operations.
//!
//! ## Usage
//!
//! ```rust
//! use tastytrade::prelude::*;
//! ```
//!
//! This will import all the commonly used types, traits, and functions.

// Re-export the main client
pub use crate::api::client::TastyTrade;

// Re-export result types
pub use crate::api::base::TastyResult;

// Re-export error types
pub use crate::error::{ApiError, DxFeedError, TastyTradeError};

// Re-export account types
pub use crate::api::accounts::{Account, AccountDetails, AccountInner, AccountNumber};

// Re-export order types
pub use crate::types::order::{
    Action, AsSymbol, LiveOrderRecord, Order, OrderBuilder, OrderId, OrderLeg, OrderLegBuilder,
    OrderPlacedResult, OrderStatus, OrderType, PriceEffect, Symbol, TimeInForce,
};

// Re-export position types
pub use crate::types::position::{BriefPosition, FullPosition, QuantityDirection};

// Re-export balance types
pub use crate::types::balance::{Balance, BalanceSnapshot, SnapshotTimeOfDay};

// Re-export instrument types
pub use crate::types::instrument::{
    Cryptocurrency, DestinationVenueSymbol, EquityInstrument, EquityInstrumentInfo, EquityOption,
    Expiration, Future, FutureOption, FutureOptionProduct, FutureProduct, FutureRoll,
    InstrumentType, NestedOptionChain, QuantityDecimalPrecision, Strike, SymbolEntry, TickSize,
    Warrant,
};

// Re-export DxFeed types
pub use crate::types::dxfeed::*;

// Re-export streaming types
pub use crate::streaming::account_streaming::{
    AccountEvent, AccountMessage, AccountStreamer, ErrorMessage, StatusMessage,
};
pub use crate::streaming::quote_streamer::{QuoteStreamer, QuoteSubscription};

// Re-export quote streaming types
pub use crate::api::quote_streaming::{DxFeedSymbol, QuoteStreamerTokens};

// Re-export option chain types
pub use crate::api::option_chain::{
    Expiration as OptionExpiration, NestedOptionChain as OptionNestedChain, OptionChain,
    OptionInfo, Strike as OptionStrike,
};

// Re-export utility types
pub use crate::utils::{
    config::TastyTradeConfig, download::*, file::*, logger::setup_logger, parse::*,
};

// Re-export login types
pub use crate::types::login::{LoginCredentials, LoginResponse, LoginResponseUser};

// Re-export event types
pub use crate::types::event::TastyEvent;
