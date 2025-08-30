use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::os::raw::{c_int, c_uint};
use strum_macros::EnumString;
use thiserror::Error;
use widestring::WideCString;

pub use libdxfeed_sys::*;

////////////////////////////////////////////////////////////////////////////////
// Trade event macros from EventData.h
////////////////////////////////////////////////////////////////////////////////
pub const DXF_ET_TRADE: c_int = 1 << dx_event_id_dx_eid_trade;
/// Quote event
pub const DXF_ET_QUOTE: c_int = 1 << dx_event_id_dx_eid_quote;
/// Summary event
pub const DXF_ET_SUMMARY: c_int = 1 << dx_event_id_dx_eid_summary;
/// Profile event
pub const DXF_ET_PROFILE: c_int = 1 << dx_event_id_dx_eid_profile;
/// Order event
pub const DXF_ET_ORDER: c_int = 1 << dx_event_id_dx_eid_order;
/// Time & sale event
pub const DXF_ET_TIME_AND_SALE: c_int = 1 << dx_event_id_dx_eid_time_and_sale;
/// Candle event
pub const DXF_ET_CANDLE: c_int = 1 << dx_event_id_dx_eid_candle;
/// Trade eth event
pub const DXF_ET_TRADE_ETH: c_int = 1 << dx_event_id_dx_eid_trade_eth;
/// Spread order event
pub const DXF_ET_SPREAD_ORDER: c_int = 1 << dx_event_id_dx_eid_spread_order;
/// Greeks event
pub const DXF_ET_GREEKS: c_int = 1 << dx_event_id_dx_eid_greeks;
/// Theo price event
pub const DXF_ET_THEO_PRICE: c_int = 1 << dx_event_id_dx_eid_theo_price;
/// Underlying event
pub const DXF_ET_UNDERLYING: c_int = 1 << dx_event_id_dx_eid_underlying;
/// Series event
pub const DXF_ET_SERIES: c_int = 1 << dx_event_id_dx_eid_series;
/// Configuration event
pub const DXF_ET_CONFIGURATION: c_int = 1 << dx_event_id_dx_eid_configuration;
pub const DXF_ET_UNUSED: c_uint = !((1 << dx_event_id_dx_eid_count) - 1);

#[derive(
    Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd, Copy, Clone, Debug, Hash, EnumString,
)]
pub enum EventType {
    Trade = DXF_ET_TRADE as isize,
    Quote = DXF_ET_QUOTE as isize,
    Summary = DXF_ET_SUMMARY as isize,
    Profile = DXF_ET_PROFILE as isize,
    Order = DXF_ET_ORDER as isize,
    TimeAndSale = DXF_ET_TIME_AND_SALE as isize,
    Candle = DXF_ET_CANDLE as isize,
    TradeETH = DXF_ET_TRADE_ETH as isize,
    SpreadOrder = DXF_ET_SPREAD_ORDER as isize,
    Greeks = DXF_ET_GREEKS as isize,
    TheoPrice = DXF_ET_THEO_PRICE as isize,
    Underlying = DXF_ET_UNDERLYING as isize,
    Series = DXF_ET_SERIES as isize,
    Configuration = DXF_ET_CONFIGURATION as isize,
}

impl TryFrom<c_int> for EventType {
    type Error = Error;

    fn try_from(value: c_int) -> Result<Self, Self::Error> {
        match value {
            DXF_ET_TRADE => Ok(EventType::Trade),
            DXF_ET_QUOTE => Ok(EventType::Quote),
            DXF_ET_SUMMARY => Ok(EventType::Summary),
            DXF_ET_PROFILE => Ok(EventType::Profile),
            DXF_ET_ORDER => Ok(EventType::Order),
            DXF_ET_TIME_AND_SALE => Ok(EventType::TimeAndSale),
            DXF_ET_CANDLE => Ok(EventType::Candle),
            DXF_ET_TRADE_ETH => Ok(EventType::TradeETH),
            DXF_ET_SPREAD_ORDER => Ok(EventType::SpreadOrder),
            DXF_ET_GREEKS => Ok(EventType::Greeks),
            DXF_ET_THEO_PRICE => Ok(EventType::TheoPrice),
            DXF_ET_UNDERLYING => Ok(EventType::Underlying),
            DXF_ET_SERIES => Ok(EventType::Series),
            DXF_ET_CONFIGURATION => Ok(EventType::Configuration),
            _ => Err(Error::Invalid(value)),
        }
    }
}

impl<T: AsRef<EventData>> From<T> for EventType {
    fn from(event: T) -> Self {
        match event.as_ref() {
            EventData::Trade(_) => EventType::Trade,
            EventData::Quote(_) => EventType::Quote,
            EventData::Summary(_) => EventType::Summary,
            EventData::Profile(_) => EventType::Profile,
            EventData::Order(_) => EventType::Order,
            EventData::TimeAndSale(_) => EventType::TimeAndSale,
            EventData::Candle(_) => EventType::Candle,
            EventData::TradeETH(_) => EventType::TradeETH,
            EventData::SpreadOrder(_) => EventType::SpreadOrder,
            EventData::Greeks(_) => EventType::Greeks,
            EventData::TheoPrice(_) => EventType::TheoPrice,
            EventData::Underlying(_) => EventType::Underlying,
            EventData::Series(_) => EventType::Series,
            EventData::Configuration(_) => EventType::Configuration,
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl EventType {
    pub fn to_string(value: c_int) -> String {
        Self::try_from(value).map_or_else(
            |_err| format!("<Unknown>({})", value),
            |event_type| format!("{}", event_type),
        )
    }
}

// A Rustified dxf_profile_t. namely for converting non-serializable raw C strings (pointers) to
// Strings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileEventData {
    ///  The correlation coefficient of the instrument to the S&P500 index (calculated, or received from other data providers)
    pub beta: f64,

    /// Earnings per share (the company’s profits divided by the number of shares). The value comes
    /// directly from the annual quarterly accounting reports of companies. Available generally for
    /// stocks
    pub eps: f64,

    /// Frequency of cash dividends payments per year (calculated)"]
    pub div_freq: f64,

    /// The amount of the last paid dividend
    pub exd_div_amount: f64,

    /// Date of the last dividend payment
    pub exd_div_date: i32,

    /// Maximal (high) price in last 52 weeks
    pub high_52_week_price: f64,

    /// Minimal (low) price in last 52 weeks
    pub low_52_week_price: f64,

    /// Shares outstanding. In general, this is the total number of shares issued by this company (only for stocks)
    pub shares: f64,

    /// The number of shares outstanding that are available to the public for trade. This field always has NaN value.
    pub free_float: f64,

    /// Maximal (high) allowed price
    pub high_limit_price: f64,

    /// Minimal (low) allowed price
    pub low_limit_price: f64,

    /// Starting time of the trading halt interval
    pub halt_start_time: i64,

    /// Ending time of the trading halt interval
    pub halt_end_time: i64,

    /// This field contains several individual flags encoded as an integer number the following way:
    ///     31...4  3210
    ///     SSR     Status
    ///  1. SSR (shortSaleRestriction) - special mode of protection against \"shorting the market\", this field
    ///     is optional. #dxf_short_sale_restriction_t
    ///  2. Status (tradingStatus) - the state of the instrument.
    pub raw_flags: i32,

    /// Description of the security instrument
    pub description: String,

    /// Description of the reason that trading was halted
    pub status_reason: String,

    /// Trading status of the security instrument
    pub trading_status: u32,

    /// Short sale restriction of the security instrument
    pub ssr: u32,
}

// impl <T: AsRef<dxf_profile_t>> From<T> for ProfileEventData {
impl From<&dxf_profile_t> for ProfileEventData {
    fn from(c_profile: &dxf_profile_t) -> Self {
        let description = unsafe {
            WideCString::from_ptr_str(c_profile.description as *const _).to_string_lossy()
        };
        let status_reason = unsafe {
            WideCString::from_ptr_str(c_profile.status_reason as *const _).to_string_lossy()
        };
        Self {
            beta: c_profile.beta,
            eps: c_profile.eps,
            div_freq: c_profile.div_freq,
            exd_div_amount: c_profile.exd_div_amount,
            exd_div_date: c_profile.exd_div_date,
            high_52_week_price: c_profile.high_52_week_price,
            low_52_week_price: c_profile.low_52_week_price,
            shares: c_profile.shares,
            free_float: c_profile.free_float,
            high_limit_price: c_profile.high_limit_price,
            low_limit_price: c_profile.low_limit_price,
            halt_start_time: c_profile.halt_start_time,
            halt_end_time: c_profile.halt_end_time,
            raw_flags: c_profile.raw_flags,
            description,
            status_reason,
            trading_status: c_profile.trading_status,
            ssr: c_profile.ssr,
        }
    }
}

//  dxf_order_t, but dealing with the string-containingan
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderEventData {
    /// Source of this order
    pub source: [dxf_char_t; 17usize],
    /// Transactional event flags.
    pub event_flags: dxf_event_flags_t,
    /// Unique per-symbol index of this order.
    pub index: dxf_long_t,
    /// Time of this order. Time is measured in milliseconds between the current time and midnight, January 1, 1970 UTC.
    pub time: dxf_long_t,
    /// Sequence number of this order to distinguish orders that have the same #time.
    pub sequence: dxf_int_t,
    /// Microseconds and nanoseconds part of time of this order.
    pub time_nanos: dxf_int_t,
    /// Order action if available, otherwise - dxf_oa_undefined. This field is a part of the FOB (\
    pub action: dxf_order_action_t,
    /// Time of the last \\ref dxf_order.action if available, otherwise - 0. This field is a part of the FOB (\
    pub action_time: dxf_long_t,
    /// Contains order ID if available, otherwise - 0. Some actions dxf_oa_trade, dxf_oa_bust have no order since they are not related\n to any order in Order book.\n\n This field is a part of the FOB (\
    pub order_id: dxf_long_t,
    /// Contains auxiliary order ID if available, otherwise - 0:\n - in dxf_oa_new - ID of the order replaced by this new order\n - in dxf_oa_delete - ID of the order that replaces this deleted order\n - in dxf_oa_partial - ID of the aggressor order\n - in dxf_oa_execute - ID of the aggressor order\n\n This field is a part of the FOB (\"Full Order Book\") support.
    pub aux_order_id: dxf_long_t,
    /// Price of this order.
    pub price: dxf_double_t,
    /// Size of this order
    pub size: dxf_double_t,
    /// Executed size of this order. This field is a part of the FOB (\
    pub executed_size: dxf_double_t,
    /// Number of individual orders in this aggregate order.
    pub count: dxf_double_t,
    /// Contains trade (order execution) ID for events containing trade-related action if available, otherwise - 0.\n\n This field is a part of the FOB (\
    pub trade_id: dxf_long_t,
    /// Contains trade price for events containing trade-related action.\n\n This field is a part of the FOB (\
    pub trade_price: dxf_double_t,
    /// Contains trade size for events containing trade-related action.\n\n This field is a part of the FOB (\
    pub trade_size: dxf_double_t,
    /// Exchange code of this order
    pub exchange_code: dxf_char_t,
    /// Side of this order
    pub side: dxf_order_side_t,
    /// Scope of this order
    pub scope: dxf_order_scope_t,
    /// Market maker or spread order
    pub mm_or_spread: String,
}

impl From<&dxf_order_t> for OrderEventData {
    fn from(c_order: &dxf_order_t) -> Self {
        let mm_or_spread = unsafe {
            WideCString::from_ptr_str(c_order.__bindgen_anon_1.market_maker as *const _)
                .to_string_lossy()
        };
        Self {
            source: c_order.source,
            event_flags: c_order.event_flags,
            index: c_order.index,
            time: c_order.time,
            sequence: c_order.sequence,
            time_nanos: c_order.time_nanos,
            action: c_order.action,
            action_time: c_order.action_time,
            order_id: c_order.order_id,
            aux_order_id: c_order.aux_order_id,
            price: c_order.price,
            size: c_order.size,
            executed_size: c_order.executed_size,
            count: c_order.count,
            trade_id: c_order.trade_id,
            trade_price: c_order.trade_price,
            trade_size: c_order.trade_size,
            exchange_code: c_order.exchange_code,
            side: c_order.side,
            scope: c_order.scope,
            mm_or_spread,
        }
    }
}

// dxf_time_and_sale / dxf_time_and_sale_t
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TimeAndSaleData {
    /// Transactional event flags. See: #dxf_event_flag
    pub event_flags: dxf_event_flags_t,
    /// Unique per-symbol index of this time and sale event
    pub index: dxf_long_t,
    /// Timestamp of the original event
    pub time: dxf_long_t,
    /// Exchange code of this time and sale event
    pub exchange_code: dxf_char_t,
    /// Price of this time and sale event
    pub price: dxf_double_t,
    /// Size of this time and sale event
    pub size: dxf_double_t,
    /// The current bid price on the market when this time and sale event had occurred
    pub bid_price: dxf_double_t,
    /// The current ask price on the market when this time and sale event had occurred
    pub ask_price: dxf_double_t,
    /// Sale conditions provided for this event by data feed. [TimeAndSale Sale
    /// Conditions](https://kb.dxfeed.com/display/DS/TimeAndSale+Sale+Conditions)
    pub exchange_sale_conditions: String,
    /// This field contains several individual flags encoded as an integer number (i.e. it's
    /// redundant with other fields here)
    /// See https://docs.dxfeed.com/c-api/structdxf__time__and__sale.html#a758b5d02999b81b6e3e8143fd0ceb0fb
    pub raw_flags: dxf_int_t,
    /// Buyer of this time and sale event
    pub buyer: String,
    /// Seller of this time and sale event
    pub seller: String,
    /// Aggressor side of this time and sale event
    pub side: dxf_order_side_t,
    /// Type of this time and sale event
    pub kind: dxf_tns_type_t,
    /// Whether this event represents a valid intraday tick
    pub is_valid_tick: bool,
    /// Whether this event represents an extended trading hours sale
    pub is_eth_trade: bool,
    /// TradeThroughExempt flag of this time and sale event
    pub trade_through_exempt: dxf_char_t,
    /// Whether this event represents a spread leg
    pub is_spread_leg: bool,
    /// Scope of this TimeAndSale.\n\n Possible values: #dxf_osc_composite (TimeAndSale events) , #dxf_osc_regional (TimeAndSale& events)
    pub scope: dxf_order_scope_t,
}

impl From<&dxf_time_and_sale_t> for TimeAndSaleData {
    fn from(c_time_and_sale: &dxf_time_and_sale_t) -> Self {
        let exchange_sale_conditions = unsafe {
            WideCString::from_ptr_str(c_time_and_sale.exchange_sale_conditions as *const _)
                .to_string_lossy()
        };
        let buyer = unsafe {
            WideCString::from_ptr_str(c_time_and_sale.buyer as *const _).to_string_lossy()
        };
        let seller = unsafe {
            WideCString::from_ptr_str(c_time_and_sale.seller as *const _).to_string_lossy()
        };
        Self {
            event_flags: c_time_and_sale.event_flags,
            index: c_time_and_sale.index,
            time: c_time_and_sale.time,
            exchange_code: c_time_and_sale.exchange_code,
            price: c_time_and_sale.price,
            size: c_time_and_sale.size,
            bid_price: c_time_and_sale.bid_price,
            ask_price: c_time_and_sale.ask_price,
            exchange_sale_conditions,
            raw_flags: c_time_and_sale.raw_flags,
            buyer,
            seller,
            side: c_time_and_sale.side,
            kind: c_time_and_sale.type_,
            is_valid_tick: c_time_and_sale.is_valid_tick > 0,
            is_eth_trade: c_time_and_sale.is_eth_trade > 0,
            trade_through_exempt: c_time_and_sale.trade_through_exempt,
            is_spread_leg: c_time_and_sale.is_spread_leg > 0,
            scope: c_time_and_sale.scope,
        }
    }
}

// dx_spread_order_t
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpreadOrderData {
    pub index: dxf_int_t,
    pub time: dxf_int_t,
    pub time_nanos: dxf_int_t,
    pub sequence: dxf_int_t,
    pub action_time: dxf_long_t,
    pub order_id: dxf_long_t,
    pub aux_order_id: dxf_long_t,
    pub price: dxf_double_t,
    pub size: dxf_double_t,
    pub executed_size: dxf_double_t,
    pub count: dxf_double_t,
    pub flags: dxf_int_t,
    pub trade_id: dxf_long_t,
    pub trade_price: dxf_double_t,
    pub trade_size: dxf_double_t,
    pub spread_symbol: String,
}

impl From<&dx_spread_order_t> for SpreadOrderData {
    fn from(c_spread_order: &dx_spread_order_t) -> Self {
        let spread_symbol = unsafe {
            WideCString::from_ptr_str(c_spread_order.spread_symbol as *const _).to_string_lossy()
        };
        Self {
            index: c_spread_order.index,
            time: c_spread_order.time,
            time_nanos: c_spread_order.time_nanos,
            sequence: c_spread_order.sequence,
            action_time: c_spread_order.action_time,
            order_id: c_spread_order.order_id,
            aux_order_id: c_spread_order.aux_order_id,
            price: c_spread_order.price,
            size: c_spread_order.size,
            executed_size: c_spread_order.executed_size,
            count: c_spread_order.count,
            flags: c_spread_order.flags,
            trade_id: c_spread_order.trade_id,
            trade_price: c_spread_order.trade_price,
            trade_size: c_spread_order.trade_size,
            spread_symbol,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationData {
    pub version: dxf_int_t,
    pub object: String,
}

impl From<&dxf_configuration_t> for ConfigurationData {
    fn from(c_config: &dxf_configuration_t) -> Self {
        let object =
            unsafe { WideCString::from_ptr_str(c_config.object as *const _).to_string_lossy() };
        Self {
            version: c_config.version,
            object,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid event_type: `{0}`")]
    Invalid(c_int),

    #[cfg(unix)]
    #[error("Converting from WideCString")]
    UtfError(#[from] widestring::error::Utf32Error),

    #[cfg(windows)]
    #[error("Converting from WideCString")]
    UtfError(#[from] widestring::error::Utf16Error),

    #[error("Unknown error")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventData {
    Trade(dxf_trade_t),
    Quote(dxf_quote_t),
    Summary(dxf_summary_t),
    Profile(ProfileEventData),
    Order(OrderEventData),
    TimeAndSale(TimeAndSaleData),
    Candle(dxf_candle_t),
    TradeETH(dxf_trade_eth_t),
    SpreadOrder(SpreadOrderData),
    Greeks(dxf_greeks_t),
    TheoPrice(dxf_theo_price_t),
    Underlying(dxf_underlying_t),
    Series(dxf_series_t),
    Configuration(ConfigurationData),
}

impl EventData {
    pub fn get_event_type(&self) -> c_int {
        match self {
            Self::Trade(_) => DXF_ET_TRADE,
            Self::Quote(_) => DXF_ET_QUOTE,
            Self::Summary(_) => DXF_ET_SUMMARY,
            Self::Profile(_) => DXF_ET_PROFILE,
            Self::Order(_) => DXF_ET_ORDER,
            Self::TimeAndSale(_) => DXF_ET_TIME_AND_SALE,
            Self::Candle(_) => DXF_ET_CANDLE,
            Self::TradeETH(_) => DXF_ET_TRADE_ETH,
            Self::SpreadOrder(_) => DXF_ET_SPREAD_ORDER,
            Self::Greeks(_) => DXF_ET_GREEKS,
            Self::TheoPrice(_) => DXF_ET_THEO_PRICE,
            Self::Underlying(_) => DXF_ET_UNDERLYING,
            Self::Series(_) => DXF_ET_SERIES,
            Self::Configuration(_) => DXF_ET_CONFIGURATION,
        }
    }
}

impl EventData {
    pub fn try_get_event_data(
        event_type: c_int,
        data: *const dxf_event_data_t,
    ) -> Result<EventData, Error> {
        match event_type {
            DXF_ET_TRADE => {
                let c_trade: &dxf_trade_t = unsafe { &*(data as *mut dxf_trade_t) };
                Ok(EventData::Trade(*c_trade))
            }
            DXF_ET_QUOTE => {
                let c_quote: &dxf_quote_t = unsafe { &*(data as *mut dxf_quote_t) };
                Ok(EventData::Quote(*c_quote))
            }
            DXF_ET_SUMMARY => {
                let c_summary: &dxf_summary_t = unsafe { &*(data as *mut dxf_summary_t) };
                Ok(EventData::Summary(*c_summary))
            }
            DXF_ET_PROFILE => {
                let c_profile: &dxf_profile_t = unsafe { &*(data as *mut dxf_profile_t) };
                Ok(EventData::Profile(ProfileEventData::from(c_profile)))
            }
            DXF_ET_ORDER => {
                let c_order: &dxf_order_t = unsafe { &*(data as *mut dxf_order_t) };
                Ok(EventData::Order(OrderEventData::from(c_order)))
            }
            DXF_ET_TIME_AND_SALE => {
                let c_time_and_sale: &dxf_time_and_sale_t =
                    unsafe { &*(data as *mut dxf_time_and_sale_t) };
                Ok(EventData::TimeAndSale(TimeAndSaleData::from(
                    c_time_and_sale,
                )))
            }
            DXF_ET_CANDLE => {
                let c_candle: &dxf_candle_t = unsafe { &*(data as *mut dxf_candle_t) };
                Ok(EventData::Candle(*c_candle))
            }
            DXF_ET_TRADE_ETH => {
                let c_trade_eth: &dxf_trade_eth_t = unsafe { &*(data as *mut dxf_trade_eth_t) };
                Ok(EventData::TradeETH(*c_trade_eth))
            }
            DXF_ET_SPREAD_ORDER => {
                let c_spread_order: &dx_spread_order = unsafe { &*(data as *mut dx_spread_order) };
                Ok(EventData::SpreadOrder(SpreadOrderData::from(
                    c_spread_order,
                )))
            }
            DXF_ET_GREEKS => {
                let c_greeks: &dxf_greeks_t = unsafe { &*(data as *mut dxf_greeks_t) };
                Ok(EventData::Greeks(*c_greeks))
            }
            DXF_ET_THEO_PRICE => {
                let c_theo: &dxf_theo_price_t = unsafe { &*(data as *mut dxf_theo_price_t) };
                Ok(EventData::TheoPrice(*c_theo))
            }
            DXF_ET_UNDERLYING => {
                let c_underlying: &dxf_underlying_t = unsafe { &*(data as *mut dxf_underlying_t) };
                Ok(EventData::Underlying(*c_underlying))
            }
            DXF_ET_SERIES => {
                let c_series: &dxf_series_t = unsafe { &*(data as *mut dxf_series_t) };
                Ok(EventData::Series(*c_series))
            }
            DXF_ET_CONFIGURATION => {
                let c_configuration: &dxf_configuration_t =
                    unsafe { &*(data as *mut dxf_configuration_t) };
                Ok(EventData::Configuration(ConfigurationData::from(
                    c_configuration,
                )))
            }
            _ => Err(Error::Invalid(event_type)),
        }
    }
}

unsafe impl Send for EventData {}
unsafe impl Sync for EventData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub sym: String,
    pub data: EventData,
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}

impl Event {
    pub fn new(sym: String, data: EventData) -> Self {
        Event { sym, data }
    }

    pub fn try_from_c(
        event_type: c_int,
        raw_sym: dxf_const_string_t,
        data: *const dxf_event_data_t,
    ) -> Result<Self, Error> {
        let c_sym = unsafe { WideCString::from_ptr_str(raw_sym as *const _) };
        let sym = c_sym.to_string()?;
        let event_data = EventData::try_get_event_data(event_type, data)?;
        Ok(Event::new(sym, event_data))
    }
}
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn event_type_from_str() {
        let name2expected: [(&str, EventType); 14] = [
            ("Trade", EventType::Trade),
            ("Quote", EventType::Quote),
            ("Summary", EventType::Summary),
            ("Profile", EventType::Profile),
            ("Order", EventType::Order),
            ("TimeAndSale", EventType::TimeAndSale),
            ("Candle", EventType::Candle),
            ("TradeETH", EventType::TradeETH),
            ("SpreadOrder", EventType::SpreadOrder),
            ("Greeks", EventType::Greeks),
            ("TheoPrice", EventType::TheoPrice),
            ("Underlying", EventType::Underlying),
            ("Series", EventType::Series),
            ("Configuration", EventType::Configuration),
        ];
        for (name, expected) in name2expected {
            let result = EventType::from_str(name);
            assert_eq!(result, Ok(expected));
        }
    }
}
