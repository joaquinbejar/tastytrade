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
pub use crate::error::{ApiError, DxFeedError, Environment, RequestContext, TastyTradeError};

// Re-export account types
pub use crate::api::accounts::{
    Account, AccountDetails, AccountInner, AccountNumber, DryRunReceipt, ReviewedOrder,
};

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

// Re-export forward-compatible wire enums
pub use crate::types::wire::{ExerciseStyle, ExpirationType, SettlementType};

// Re-export DxFeed types
pub use crate::types::dxfeed::*;

// Re-export streaming types
pub use crate::streaming::account_streaming::{
    AccountEvent, AccountMessage, AccountStreamer, AccountTransport, ErrorMessage, StatusMessage,
};
pub use crate::streaming::quote_streamer::{QuoteStreamer, QuoteSubscription};
pub use crate::streaming::reconnect::{BackoffPolicy, ConnectionState};

// Re-export quote streaming types
pub use crate::api::quote_streaming::{DxFeedSymbol, QuoteStreamerTokens};

// Re-export option chain types
pub use crate::api::option_chain::{
    Expiration as OptionExpiration, NestedOptionChain as OptionNestedChain, OptionChain,
    OptionInfo, Strike as OptionStrike,
};

// Re-export utility types
pub use crate::utils::{
    config::TastyTradeConfig,
    download::*,
    file::*,
    logger::{LoggerInit, setup_logger, try_setup_logger, try_setup_logger_with_level},
    parse::*,
};

// Re-export OAuth2 types
pub use crate::api::oauth::OAuthSession;
pub use crate::types::oauth::{
    AccessToken, AuthorizationCode, AuthorizationRequest, ClientSecret, IdToken, OAuthGrant,
    RefreshToken, Scope, TokenResponse,
};

// Re-export event types
pub use crate::types::event::TastyEvent;
