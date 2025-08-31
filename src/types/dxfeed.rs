//! Internal DXFeed types to replace external dxfeed dependency
//! This module contains the essential types and constants needed for quote streaming

use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};

// Event type flags - these are bit flags used to identify different event types
pub const DXF_ET_QUOTE: i32 = 0x01;
pub const DXF_ET_TRADE: i32 = 0x02;
pub const DXF_ET_GREEKS: i32 = 0x08;

/// Represents a quote event from the market data feed
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
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
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
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
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
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
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub enum EventData {
    Quote(DxfQuoteT),
    Trade(DxfTradeT),
    Greeks(DxfGreeksT),
}

/// Main event structure that contains symbol and event data
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DXF_ET_QUOTE, 0x01);
        assert_eq!(DXF_ET_TRADE, 0x02);
        assert_eq!(DXF_ET_GREEKS, 0x08);
    }

    #[test]
    fn test_dxf_quote_t_default() {
        let quote = DxfQuoteT::default();
        assert_eq!(quote.time, 0);
        assert_eq!(quote.sequence, 0);
        assert_eq!(quote.bid_price, 0.0);
        assert_eq!(quote.ask_price, 0.0);
        assert_eq!(quote.bid_size, 0);
        assert_eq!(quote.ask_size, 0);
    }

    #[test]
    fn test_dxf_trade_t_default() {
        let trade = DxfTradeT::default();
        assert_eq!(trade.time, 0);
        assert_eq!(trade.price, 0.0);
        assert_eq!(trade.size, 0);
        assert_eq!(trade.exchange_code, 0);
        assert_eq!(trade.day_volume, 0.0);
    }

    #[test]
    fn test_dxf_greeks_t_default() {
        let greeks = DxfGreeksT::default();
        assert_eq!(greeks.event_flags, 0);
        assert_eq!(greeks.delta, 0.0);
        assert_eq!(greeks.gamma, 0.0);
        assert_eq!(greeks.theta, 0.0);
        assert_eq!(greeks.vega, 0.0);
        assert_eq!(greeks.rho, 0.0);
    }

    #[test]
    fn test_event_new_quote() {
        let quote = DxfQuoteT {
            bid_price: 100.0,
            ask_price: 101.0,
            bid_size: 100,
            ask_size: 200,
            ..Default::default()
        };
        
        let event = Event::new_quote("AAPL".to_string(), quote);
        assert_eq!(event.sym, "AAPL");
        
        match event.data {
            EventData::Quote(q) => {
                assert_eq!(q.bid_price, 100.0);
                assert_eq!(q.ask_price, 101.0);
                assert_eq!(q.bid_size, 100);
                assert_eq!(q.ask_size, 200);
            },
            _ => panic!("Expected Quote event data"),
        }
    }

    #[test]
    fn test_event_new_trade() {
        let trade = DxfTradeT {
            price: 150.50,
            size: 1000,
            exchange_code: 1,
            ..Default::default()
        };
        
        let event = Event::new_trade("MSFT".to_string(), trade);
        assert_eq!(event.sym, "MSFT");
        
        match event.data {
            EventData::Trade(t) => {
                assert_eq!(t.price, 150.50);
                assert_eq!(t.size, 1000);
                assert_eq!(t.exchange_code, 1);
            },
            _ => panic!("Expected Trade event data"),
        }
    }

    #[test]
    fn test_event_new_greeks() {
        let greeks = DxfGreeksT {
            delta: 0.5,
            gamma: 0.1,
            theta: -0.05,
            vega: 0.2,
            rho: 0.03,
            volatility: 0.25,
            ..Default::default()
        };
        
        let event = Event::new_greeks("AAPL240920C00150000".to_string(), greeks);
        assert_eq!(event.sym, "AAPL240920C00150000");
        
        match event.data {
            EventData::Greeks(g) => {
                assert_eq!(g.delta, 0.5);
                assert_eq!(g.gamma, 0.1);
                assert_eq!(g.theta, -0.05);
                assert_eq!(g.vega, 0.2);
                assert_eq!(g.rho, 0.03);
                assert_eq!(g.volatility, 0.25);
            },
            _ => panic!("Expected Greeks event data"),
        }
    }

    #[test]
    fn test_serialization() {
        let quote = DxfQuoteT {
            bid_price: 100.0,
            ask_price: 101.0,
            ..Default::default()
        };
        
        let serialized = serde_json::to_string(&quote).unwrap();
        assert!(serialized.contains("100.0"));
        assert!(serialized.contains("101.0"));
        
        let deserialized: DxfQuoteT = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.bid_price, 100.0);
        assert_eq!(deserialized.ask_price, 101.0);
    }

    #[test]
    fn test_event_data_enum() {
        let quote_data = EventData::Quote(DxfQuoteT::default());
        let trade_data = EventData::Trade(DxfTradeT::default());
        let greeks_data = EventData::Greeks(DxfGreeksT::default());
        
        match quote_data {
            EventData::Quote(_) => {}, // Success
            _ => panic!("Expected Quote variant"),
        }
        
        match trade_data {
            EventData::Trade(_) => {}, // Success
            _ => panic!("Expected Trade variant"),
        }
        
        match greeks_data {
            EventData::Greeks(_) => {}, // Success
            _ => panic!("Expected Greeks variant"),
        }
    }

    #[test]
    fn test_clone_and_debug() {
        let original_quote = DxfQuoteT {
            bid_price: 50.0,
            ask_price: 51.0,
            ..Default::default()
        };
        
        let cloned_quote = original_quote.clone();
        assert_eq!(original_quote.bid_price, cloned_quote.bid_price);
        assert_eq!(original_quote.ask_price, cloned_quote.ask_price);
        
        let debug_str = format!("{:?}", original_quote);
        assert!(debug_str.contains("50.0"));
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new_quote("TEST".to_string(), DxfQuoteT::default());
        
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("TEST"));
        assert!(serialized.contains("Quote"));
        
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.sym, "TEST");
        matches!(deserialized.data, EventData::Quote(_));
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
