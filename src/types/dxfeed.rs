//! Internal DXFeed types to replace external dxfeed dependency
//! This module contains the essential types and constants needed for quote streaming

use serde::{Deserialize, Serialize};

// Event type flags - these are bit flags used to identify different event types
pub const DXF_ET_QUOTE: i32 = 0x01;
pub const DXF_ET_TRADE: i32 = 0x02;
pub const DXF_ET_GREEKS: i32 = 0x08;

/// Represents a quote event from the market data feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxfQuoteT {
    pub time: i64,
    pub sequence: i32,
    pub time_nanos: i32,
    pub bid_time: i64,
    pub bid_exchange_code: i16,
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size: i64,
    pub ask_time: i64,
    pub ask_size: i64,
    pub ask_exchange_code: i16,
    pub scope: i32,
}

/// Represents a trade event from the market data feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxfTradeT {
    pub time: i64,
    pub sequence: i32,
    pub time_nanos: i32,
    pub exchange_code: i16,
    pub price: f64,
    pub size: i64,
    pub tick: i32,
    pub change: f64,
    pub day_id: i32,
    pub day_volume: f64,
    pub day_turnover: f64,
    pub raw_flags: i32,
    pub direction: i32,
    pub is_eth: i32,
    pub scope: i32,
}

/// Represents Greeks data for options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxfGreeksT {
    pub event_flags: i32,
    pub index: i64,
    pub time: i64,
    pub price: f64,
    pub volatility: f64,
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub rho: f64,
    pub vega: f64,
}

/// Enum representing different types of market event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventData {
    Quote(DxfQuoteT),
    Trade(DxfTradeT),
    Greeks(DxfGreeksT),
}

/// Main event structure that contains symbol and event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub sym: String,
    pub data: EventData,
}

impl Event {
    /// Create a new quote event
    pub fn new_quote(symbol: String, quote: DxfQuoteT) -> Self {
        Self {
            sym: symbol,
            data: EventData::Quote(quote),
        }
    }

    /// Create a new trade event
    pub fn new_trade(symbol: String, trade: DxfTradeT) -> Self {
        Self {
            sym: symbol,
            data: EventData::Trade(trade),
        }
    }

    /// Create a new Greeks event
    pub fn new_greeks(symbol: String, greeks: DxfGreeksT) -> Self {
        Self {
            sym: symbol,
            data: EventData::Greeks(greeks),
        }
    }
}

/// Default implementations for the data structures
impl Default for DxfQuoteT {
    fn default() -> Self {
        Self {
            time: 0,
            sequence: 0,
            time_nanos: 0,
            bid_time: 0,
            bid_exchange_code: 0,
            bid_price: 0.0,
            ask_price: 0.0,
            bid_size: 0,
            ask_time: 0,
            ask_size: 0,
            ask_exchange_code: 0,
            scope: 0,
        }
    }
}

impl Default for DxfTradeT {
    fn default() -> Self {
        Self {
            time: 0,
            sequence: 0,
            time_nanos: 0,
            exchange_code: 0,
            price: 0.0,
            size: 0,
            tick: 0,
            change: 0.0,
            day_id: 0,
            day_volume: 0.0,
            day_turnover: 0.0,
            raw_flags: 0,
            direction: 0,
            is_eth: 0,
            scope: 0,
        }
    }
}

impl Default for DxfGreeksT {
    fn default() -> Self {
        Self {
            event_flags: 0,
            index: 0,
            time: 0,
            price: 0.0,
            volatility: 0.0,
            delta: 0.0,
            gamma: 0.0,
            theta: 0.0,
            rho: 0.0,
            vega: 0.0,
        }
    }
}
