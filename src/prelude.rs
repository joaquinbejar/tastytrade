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

// Re-export the customer resource. Personal data: nothing here renders itself.
pub use crate::types::customer::{
    Customer, CustomerAddress, CustomerEntity, CustomerPerson, CustomerSuitability, EntityOfficer,
    EntitySuitability,
};

// Re-export account types
pub use crate::api::accounts::{
    Account, AccountDetails, AccountInner, AccountNumber, DryRunReceipt, ReviewedOrder,
};

// Re-export order types
pub use crate::types::order::{
    Action, AsSymbol, LiveOrderLeg, LiveOrderRecord, Order, OrderBuilder, OrderFill, OrderId,
    OrderLeg, OrderLegBuilder, OrderPlacedResult, OrderStatus, OrderType, PriceEffect, Symbol,
    TimeInForce,
};

// Re-export the venue-level capability switches
pub use crate::types::capability::{
    CRYPTOCURRENCY_TRADING_ENABLED, CRYPTOCURRENCY_TRADING_SOURCE,
    CRYPTOCURRENCY_TRADING_SUSPENDED_ON,
};

// Re-export complex orders
pub use crate::api::accounts::{
    ComplexOrderReceipt, PairsThresholdReceipt, ReviewedComplexOrder, ReviewedPairsThreshold,
};
pub use crate::types::complex_order::{
    ComplexOrder, ComplexOrderComponent, ComplexOrderId, ComplexOrderRequest, ComplexOrderType,
    PairsThresholdEdit, RatioPriceComparator, RelatedOrder,
};

// Re-export the order lifecycle
pub use crate::api::accounts::{AmendmentIntent, AmendmentReceipt, ReviewedAmendment};
pub use crate::types::order::OrderAmendment;
pub use crate::types::order_filter::{
    CustomerLiveOrderFilter, CustomerOrderFilter, LiveOrderFilter, OrderFilter, OrderSort,
};

// Re-export position types
pub use crate::types::position::{BriefPosition, FullPosition, QuantityDirection};

// Re-export the payload types the account streamer shares with REST
pub use crate::types::quote_alert::{
    NewQuoteAlert, QuoteAlert, QuoteAlertField, QuoteAlertOperator,
};
pub use crate::types::watchlist::{NewWatchlist, PairsWatchlist, Watchlist, WatchlistEntry};

// Re-export balance types
pub use crate::types::balance::{Balance, BalanceSnapshot, SnapshotTimeOfDay};

// Re-export market metrics, dividends and earnings
pub use crate::types::market_metrics::{
    DividendReport, EarningsRange, EarningsReport, ExpirationImpliedVolatility, MarketMetric,
};

// Re-export market sessions and holidays
pub use crate::types::market_time::{
    CurrentMarketSession, FuturesExchange, InstrumentCollection, MAX_SESSION_RANGE_MONTHS,
    MarketCalendar, MarketSession, SessionCollection, SessionRange,
};

// Re-export the REST market-data snapshot
pub use crate::types::market_data::{
    MAX_MARKET_DATA_SYMBOLS, MarketDataRequest, MarketDataSnapshot,
};

// Re-export the margin and risk-parameter surface
pub use crate::types::margin::{
    EffectiveMarginRequirement, MAX_MARGIN_LEGS, MarginConfiguration, MarginDryRunLeg,
    MarginDryRunOrder, MarginEstimate, MarginGroup, MarginGroupResult, MarginImpact,
    MarginOrderLeg, MarginOrderRequest, MarginPositionEntry, MarginRequirementsReport,
    MarginStrategy, MarginStrategyGroup, NetLiquidatingValues, PositionLimit, SpanExchange,
    SpanRow,
};

// Re-export the equity curve
pub use crate::types::net_liq::{NetLiqHistoryFilter, NetLiqOhlc, NetLiqRange, TimeBack};

// Re-export the account trading status
pub use crate::types::trading_status::TradingStatus;

// Re-export the account ledger
pub use crate::types::transaction::{
    TotalFees, Transaction, TransactionAction, TransactionFilter, TransactionSort,
    TransactionSubType, TransactionType, TransactionTypes,
};

// Re-export the account-scoped filters
pub use crate::types::account_filter::{BalanceSnapshotFilter, PositionFilter, SnapshotRange};

// Re-export instrument types
pub use crate::types::instrument::{
    Cryptocurrency, DestinationVenueSymbol, EquityInstrument, EquityInstrumentInfo, EquityOption,
    Expiration, Future, FutureOption, FutureOptionProduct, FutureProduct, FutureRoll,
    InstrumentType, NestedOptionChain, QuantityDecimalPrecision, Strike, SymbolEntry, TickSize,
    Warrant,
};

// Re-export the typed listing filters and the request side of pagination
pub use crate::api::base::{Items, Paginated, Pagination};
pub use crate::api::query::PageRequest;
pub use crate::types::instrument_filter::{ActiveEquityFilter, EquityFilter, FutureFilter};

// Re-export the search surface
pub use crate::types::search::{
    AiSearchToken, InstrumentSearchFilter, InstrumentSearchResult, MAX_SEARCH_RESULTS,
    SymbolSearchResult,
};

// Re-export forward-compatible wire enums
pub use crate::types::wire::{ExerciseStyle, ExpirationType, Lendability, SettlementType};

// Re-export DxFeed types
pub use crate::types::dxfeed::*;

// Re-export streaming types
pub use crate::streaming::account_streaming::{
    AccountEvent, AccountNotification, AccountStreamer, AccountTransport, ErrorMessage,
    NotificationPayload, RawPayload, StatusMessage, SubRequestAction, UnknownEvent,
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
