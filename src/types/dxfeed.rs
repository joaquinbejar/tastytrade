//! Internal DXFeed types to replace external dxfeed dependency
//! This module contains the essential types and constants needed for quote streaming

use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};

/// One market event type a subscription can ask for.
///
/// Replaces the `DXF_ET_*` bitmask. Those were three constants out of a C
/// library's flag set, and a caller had to know which bit meant Greeks; the
/// set is now eleven and closed, so a typo is a compile error rather than a
/// subscription that silently asks for nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EventKind {
    /// Top of book: best bid and ask with their sizes.
    Quote,
    /// A trade print from the regular session.
    Trade,
    /// A trade print from the pre- or post-market session.
    ///
    /// Between 04:00–09:30 and 16:00–20:00 ET, [`EventKind::Trade`] is silent
    /// and this is the only print there is. Without it there is no route to an
    /// extended-hours last price anywhere in this crate.
    TradeEth,
    /// Option risk measures computed by the feed.
    Greeks,
    /// One OHLC bar. Needs a period and a start time; see
    /// [`CandlePeriod`] and `QuoteSubscription::add_candles`.
    Candle,
    /// The day's open, extremes and previous close.
    Summary,
    /// One execution as it printed, with the quote around it.
    TimeAndSale,
    /// Instrument metadata: description, trading status, fundamentals.
    Profile,
    /// The option surface over an underlying.
    Underlying,
    /// A theoretical option price with the inputs behind it.
    TheoPrice,
    /// One option expiration's computed values for an underlying.
    Series,
}

impl EventKind {
    /// Every event type this crate routes.
    ///
    /// Ordered as the feed's own enum is, which keeps the extended-hours print
    /// ahead of the regular one — the ordering upstream depends on and this
    /// crate has no reason to disturb.
    pub const ALL: [EventKind; 11] = [
        EventKind::Quote,
        EventKind::TradeEth,
        EventKind::Trade,
        EventKind::Greeks,
        EventKind::Candle,
        EventKind::Summary,
        EventKind::TimeAndSale,
        EventKind::Profile,
        EventKind::Underlying,
        EventKind::TheoPrice,
        EventKind::Series,
    ];

    /// The name the feed uses on the wire.
    pub fn wire_name(&self) -> &'static str {
        match self {
            EventKind::Quote => "Quote",
            EventKind::Trade => "Trade",
            EventKind::TradeEth => "TradeETH",
            EventKind::Greeks => "Greeks",
            EventKind::Candle => "Candle",
            EventKind::Summary => "Summary",
            EventKind::TimeAndSale => "TimeAndSale",
            EventKind::Profile => "Profile",
            EventKind::Underlying => "Underlying",
            EventKind::TheoPrice => "TheoPrice",
            EventKind::Series => "Series",
        }
    }

    /// Whether this kind needs a period and a start time rather than a bare
    /// symbol.
    ///
    /// Only candles do. A candle subscription is addressed by a symbol that
    /// carries its own period — `AAPL{=5m}` — so two periods of one underlying
    /// are two different streamer symbols.
    pub fn needs_a_period(&self) -> bool {
        matches!(self, EventKind::Candle)
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.wire_name())
    }
}

/// The unit a candle period is counted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CandleUnit {
    /// `s`
    Seconds,
    /// `m`
    Minutes,
    /// `h`
    Hours,
    /// `d`
    Days,
    /// `w`
    Weeks,
    /// `mo`
    Months,
}

impl CandleUnit {
    /// The suffix letter the feed uses.
    pub fn as_str(&self) -> &'static str {
        match self {
            CandleUnit::Seconds => "s",
            CandleUnit::Minutes => "m",
            CandleUnit::Hours => "h",
            CandleUnit::Days => "d",
            CandleUnit::Weeks => "w",
            CandleUnit::Months => "mo",
        }
    }
}

impl std::fmt::Display for CandleUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How long one candle covers.
///
/// Rendered into the symbol as `{=<n><unit>}`, so `AAPL` at five minutes is the
/// streamer symbol `AAPL{=5m}`. Typed so a caller never builds that string by
/// hand: a malformed suffix is accepted by the venue and then delivers
/// nothing, which is indistinguishable from a quiet market.
///
/// The count is a [`NonZeroU32`](std::num::NonZeroU32) and the field is
/// private, so a zero-length period is **unrepresentable** rather than merely
/// rejected by the constructors. An earlier version was an enum with public
/// payloads, and `CandlePeriod::Minutes(0)` walked straight past the
/// validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CandlePeriod {
    count: std::num::NonZeroU32,
    unit: CandleUnit,
}

impl CandlePeriod {
    /// A period of `count` of `unit`.
    ///
    /// # Errors
    ///
    /// A zero-length candle is not a candle. It renders a suffix the venue
    /// accepts and never fills, so it is refused here instead.
    pub fn new(count: u32, unit: CandleUnit) -> crate::TastyResult<Self> {
        let count = std::num::NonZeroU32::new(count).ok_or_else(|| {
            crate::TastyTradeError::Precondition(
                "a candle period of zero is not a period; the venue accepts the suffix \
                 and then delivers nothing, which looks exactly like a quiet market"
                    .to_string(),
            )
        })?;

        Ok(Self { count, unit })
    }

    /// A period of `count` seconds.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn seconds(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Seconds)
    }

    /// A period of `count` minutes.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn minutes(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Minutes)
    }

    /// A period of `count` hours.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn hours(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Hours)
    }

    /// A period of `count` days.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn days(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Days)
    }

    /// A period of `count` weeks.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn weeks(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Weeks)
    }

    /// A period of `count` months.
    ///
    /// # Errors
    ///
    /// As [`CandlePeriod::new`].
    pub fn months(count: u32) -> crate::TastyResult<Self> {
        Self::new(count, CandleUnit::Months)
    }

    /// How many of [`CandlePeriod::unit`] this period covers.
    pub fn count(&self) -> u32 {
        self.count.get()
    }

    /// What the count is counted in.
    pub fn unit(&self) -> CandleUnit {
        self.unit
    }

    /// The `{=…}` suffix this period appends to a symbol.
    pub fn suffix(&self) -> String {
        format!("{{={}{}}}", self.count, self.unit.as_str())
    }

    /// The streamer symbol for `symbol` at this period.
    ///
    /// This is the string the venue is subscribed with **and** the
    /// `eventSymbol` the candles come back under, which is what keeps two
    /// periods of one underlying from delivering into each other.
    pub fn streamer_symbol(&self, symbol: &str) -> String {
        format!("{symbol}{}", self.suffix())
    }
}

impl std::fmt::Display for CandlePeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.suffix())
    }
}

/// The top of book for a symbol.
///
/// Prices are `f64` because that is what the feed sends. This is the one place
/// in the crate where money is not `Decimal`: converting here would imply a
/// precision the feed does not have. Convert at the boundary if you are going
/// to settle anything against these.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfQuoteT {
    /// Event timestamp, milliseconds since the Unix epoch.
    pub time: i64,
    /// Sequence number, for ordering events sharing a timestamp.
    pub sequence: i32,
    /// Sub-millisecond part of `time`, in nanoseconds.
    pub time_nanos: i32,
    /// When the bid was quoted, milliseconds since the Unix epoch.
    pub bid_time: i64,
    /// Exchange the bid came from, as a single-character code.
    pub bid_exchange_code: i16,
    /// Best bid.
    pub bid_price: f64,
    /// Best ask.
    pub ask_price: f64,
    /// Size available at the bid.
    pub bid_size: i64,
    /// When the ask was quoted, milliseconds since the Unix epoch.
    pub ask_time: i64,
    /// Size available at the ask.
    pub ask_size: i64,
    /// Exchange the ask came from, as a single-character code.
    pub ask_exchange_code: i16,
    /// Which book this quote describes: composite, regional or aggregate.
    pub scope: i32,
}

/// The last trade for a symbol, with the day's running totals.
///
/// As with [`DxfQuoteT`], the figures are `f64` because the feed sends them
/// that way.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfTradeT {
    /// Event timestamp, milliseconds since the Unix epoch.
    pub time: i64,
    /// Sequence number, for ordering events sharing a timestamp.
    pub sequence: i32,
    /// Sub-millisecond part of `time`, in nanoseconds.
    pub time_nanos: i32,
    /// Exchange the trade printed on, as a single-character code.
    pub exchange_code: i16,
    /// Trade price.
    pub price: f64,
    /// Trade size.
    pub size: i64,
    /// Uptick or downtick relative to the previous trade.
    pub tick: i32,
    /// Price change against the previous close.
    pub change: f64,
    /// Trading day, as a `YYYYMMDD` integer.
    pub day_id: i32,
    /// Shares or contracts traded so far today.
    pub day_volume: f64,
    /// Notional traded so far today.
    pub day_turnover: f64,
    /// Feed-specific flag bits, unmodelled.
    pub raw_flags: i32,
    /// Direction of the price move that produced this trade.
    pub direction: i32,
    /// Non-zero when the trade happened in extended trading hours.
    pub is_eth: i32,
    /// Which book this trade belongs to: composite or regional.
    pub scope: i32,
}

/// Option risk measures, as the feed computes them.
///
/// These are model outputs rather than quoted values: the venue's own
/// volatility surface and rate assumptions produced them, and a different
/// model gives different numbers for the same option.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfGreeksT {
    /// Feed flag bits describing the event's place in a snapshot or update.
    pub event_flags: i32,
    /// Index the feed uses to order and replace events for this symbol.
    pub index: i64,
    /// Event timestamp, milliseconds since the Unix epoch.
    pub time: i64,
    /// Option price the Greeks were computed against.
    pub price: f64,
    /// Implied volatility, as a fraction rather than a percentage.
    pub volatility: f64,
    /// Change in option price per unit change in the underlying.
    pub delta: f64,
    /// Change in delta per unit change in the underlying.
    pub gamma: f64,
    /// Change in option price per day of time decay.
    pub theta: f64,
    /// Change in option price per unit change in the interest rate.
    pub rho: f64,
    /// Change in option price per unit change in volatility.
    pub vega: f64,
}

/// One OHLC bar.
///
/// The only route to historical price data anywhere in this crate: there is no
/// REST endpoint for a price series, so a candle subscription is it.
///
/// `sym` on the surrounding [`Event`] carries the period — `AAPL{=5m}` — which
/// is what tells two periods of the same underlying apart.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfCandleT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Snapshot and transaction bits. Non-zero values delimit a historical
    /// snapshot.
    pub event_flags: i64,
    /// Unique index of the bar within its subscription.
    pub index: i64,
    /// Start of the bar, in epoch milliseconds.
    pub time: i64,
    /// Sequence number, for bars sharing a timestamp.
    pub sequence: i64,
    /// How many events were aggregated into the bar.
    pub count: i64,
    /// First price in the bar.
    pub open: f64,
    /// Highest price in the bar.
    pub high: f64,
    /// Lowest price in the bar.
    pub low: f64,
    /// Last price in the bar.
    pub close: f64,
    /// Total volume traded during the bar.
    pub volume: f64,
    /// Volume-weighted average price for the bar.
    pub vwap: f64,
    /// Volume traded at the bid.
    pub bid_volume: f64,
    /// Volume traded at the ask.
    pub ask_volume: f64,
    /// Implied volatility over the bar, for instruments that have one.
    pub imp_volatility: f64,
    /// Open interest at the end of the bar.
    pub open_interest: f64,
}

/// The trading day's open, extremes and previous close.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfSummaryT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Trading day, as a `YYYYMMDD` integer.
    pub day_id: i64,
    /// The day's opening price.
    pub day_open_price: f64,
    /// The day's high.
    pub day_high_price: f64,
    /// The day's low.
    pub day_low_price: f64,
    /// The day's close, which may be provisional; see `day_close_price_type`.
    pub day_close_price: f64,
    /// Whether the close is final, indicative or preliminary.
    pub day_close_price_type: String,
    /// The previous trading day, as a `YYYYMMDD` integer.
    pub prev_day_id: i64,
    /// The previous day's close.
    pub prev_day_close_price: f64,
    /// Whether the previous close is final, indicative or preliminary.
    pub prev_day_close_price_type: String,
    /// The previous day's volume.
    pub prev_day_volume: f64,
    /// Open interest, for instruments that have it.
    pub open_interest: f64,
}

/// One execution as it printed, with the quote around it.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfTimeAndSaleT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Snapshot and transaction bits.
    pub event_flags: i64,
    /// Unique index of the print within its subscription.
    pub index: i64,
    /// When it printed, in epoch milliseconds.
    pub time: i64,
    /// Sub-millisecond part of `time`, in nanoseconds.
    pub time_nano_part: i64,
    /// Sequence number, for prints sharing a timestamp.
    pub sequence: i64,
    /// The exchange it printed on.
    pub exchange_code: String,
    /// The price it printed at.
    pub price: f64,
    /// The size that printed.
    pub size: f64,
    /// The bid at the time of the print.
    pub bid_price: f64,
    /// The ask at the time of the print.
    pub ask_price: f64,
    /// Exchange sale conditions.
    pub exchange_sale_conditions: String,
    /// Trade-through exemption, when one applies.
    pub trade_through_exempt: String,
    /// Which side initiated the trade.
    pub aggressor_side: String,
    /// Whether the print is one leg of a spread.
    pub spread_leg: bool,
    /// Whether it printed outside regular trading hours.
    pub extended_trading_hours: bool,
    /// Whether the print counts towards the day's statistics.
    pub valid_tick: bool,
    /// The kind of sale this was.
    pub sale_type: String,
    /// The buying party, where the venue publishes it.
    pub buyer: String,
    /// The selling party, where the venue publishes it.
    pub seller: String,
}

/// Instrument metadata: description, trading status and fundamentals.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfProfileT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// The instrument's description.
    pub description: String,
    /// Any short-sale restriction in force.
    pub short_sale_restriction: String,
    /// Whether the instrument is trading, halted or otherwise restricted.
    pub trading_status: String,
    /// Why, when the status is not `Active`.
    pub status_reason: String,
    /// When a halt started, in epoch milliseconds.
    pub halt_start_time: i64,
    /// When a halt is due to end, in epoch milliseconds.
    pub halt_end_time: i64,
    /// The upper limit price for the session.
    pub high_limit_price: f64,
    /// The lower limit price for the session.
    pub low_limit_price: f64,
    /// The 52-week high.
    pub high_52_week_price: f64,
    /// The 52-week low.
    pub low_52_week_price: f64,
    /// Beta against the market.
    pub beta: f64,
    /// Earnings per share.
    pub earnings_per_share: f64,
    /// How many times a year the instrument pays a dividend.
    pub dividend_frequency: f64,
    /// The most recent ex-dividend amount.
    pub ex_dividend_amount: f64,
    /// The most recent ex-dividend day, as a `YYYYMMDD` integer.
    pub ex_dividend_day_id: i64,
    /// Shares outstanding.
    pub shares: f64,
    /// Free float.
    pub free_float: f64,
}

/// The option surface over an underlying.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfUnderlyingT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Snapshot and transaction bits.
    pub event_flags: i64,
    /// Unique index within the subscription.
    pub index: i64,
    /// Event timestamp, in epoch milliseconds.
    pub time: i64,
    /// Sequence number.
    pub sequence: i64,
    /// 30-day implied volatility.
    pub volatility: f64,
    /// Implied volatility of the front-month series.
    pub front_volatility: f64,
    /// Implied volatility of the second series.
    pub back_volatility: f64,
    /// Call option volume.
    pub call_volume: f64,
    /// Put option volume.
    pub put_volume: f64,
    /// Put volume over call volume.
    pub put_call_ratio: f64,
}

/// A theoretical option price with the inputs behind it.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfTheoPriceT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Snapshot and transaction bits.
    pub event_flags: i64,
    /// Unique index within the subscription.
    pub index: i64,
    /// Event timestamp, in epoch milliseconds.
    pub time: i64,
    /// Sequence number.
    pub sequence: i64,
    /// The theoretical price.
    pub price: f64,
    /// The underlying price it was computed against.
    pub underlying_price: f64,
    /// Delta at that price.
    pub delta: f64,
    /// Gamma at that price.
    pub gamma: f64,
    /// The dividend assumption used.
    pub dividend: f64,
    /// The interest-rate assumption used.
    pub interest: f64,
}

/// One extended-hours print.
///
/// Between 04:00–09:30 and 16:00–20:00 ET, [`DxfTradeT`] is silent and this is
/// the only print there is — so this is the only route to an extended-hours
/// last price anywhere in this crate.
///
/// A superset of [`DxfTradeT`]: every field a regular trade carries is here,
/// plus the exchange, the tick direction and the session flag.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfTradeEthT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// When it printed, in epoch milliseconds.
    pub time: i64,
    /// Sub-millisecond part of `time`, in nanoseconds.
    pub time_nano_part: i64,
    /// Sequence number.
    pub sequence: i64,
    /// The exchange it printed on.
    pub exchange_code: String,
    /// The price it printed at.
    pub price: f64,
    /// Change against the previous close.
    pub change: f64,
    /// The size that printed.
    pub size: f64,
    /// Trading day, as a `YYYYMMDD` integer.
    pub day_id: i64,
    /// Volume traded in the extended session so far.
    pub day_volume: f64,
    /// Notional traded in the extended session so far.
    pub day_turnover: f64,
    /// Whether the price moved up or down into this print.
    pub tick_direction: String,
    /// Whether this print is from an extended-hours session.
    pub extended_trading_hours: bool,
}

/// One option expiration's computed values for an underlying.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DxfSeriesT {
    /// When the server emitted the event, in epoch milliseconds.
    pub event_time: i64,
    /// Snapshot and transaction bits.
    pub event_flags: i64,
    /// Unique index within the subscription.
    pub index: i64,
    /// Event timestamp, in epoch milliseconds.
    pub time: i64,
    /// Sequence number.
    pub sequence: i64,
    /// The expiration this series describes, as a `YYYYMMDD` integer.
    pub expiration: i64,
    /// Implied volatility of the series.
    pub volatility: f64,
    /// Call option volume.
    pub call_volume: f64,
    /// Put option volume.
    pub put_volume: f64,
    /// Put volume over call volume.
    pub put_call_ratio: f64,
    /// The forward price for the expiration.
    pub forward_price: f64,
    /// The dividend assumption used.
    pub dividend: f64,
    /// The interest-rate assumption used.
    pub interest: f64,
}

/// Enum representing different types of market event data
///
/// One variant per [`EventKind`]. Adding the eight that were missing is
/// breaking for any consumer matching this exhaustively, which is the point:
/// the events were arriving and being dropped, and a consumer that thought it
/// had handled every case had not.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub enum EventData {
    /// Top of book.
    Quote(DxfQuoteT),
    /// A regular-session trade print.
    Trade(DxfTradeT),
    /// An extended-hours trade print.
    TradeEth(Box<DxfTradeEthT>),
    /// Option risk measures.
    Greeks(DxfGreeksT),
    /// One OHLC bar.
    Candle(Box<DxfCandleT>),
    /// The day's open, extremes and previous close.
    Summary(Box<DxfSummaryT>),
    /// One execution as it printed.
    TimeAndSale(Box<DxfTimeAndSaleT>),
    /// Instrument metadata and fundamentals.
    Profile(Box<DxfProfileT>),
    /// The option surface over an underlying.
    Underlying(Box<DxfUnderlyingT>),
    /// A theoretical option price.
    TheoPrice(Box<DxfTheoPriceT>),
    /// One option expiration's computed values.
    Series(Box<DxfSeriesT>),
}

impl EventData {
    /// Which event type this is.
    ///
    /// Lets a caller log or route on the kind without matching every variant.
    pub fn kind(&self) -> EventKind {
        match self {
            EventData::Quote(_) => EventKind::Quote,
            EventData::Trade(_) => EventKind::Trade,
            EventData::TradeEth(_) => EventKind::TradeEth,
            EventData::Greeks(_) => EventKind::Greeks,
            EventData::Candle(_) => EventKind::Candle,
            EventData::Summary(_) => EventKind::Summary,
            EventData::TimeAndSale(_) => EventKind::TimeAndSale,
            EventData::Profile(_) => EventKind::Profile,
            EventData::Underlying(_) => EventKind::Underlying,
            EventData::TheoPrice(_) => EventKind::TheoPrice,
            EventData::Series(_) => EventKind::Series,
        }
    }
}

/// Main event structure that contains symbol and event data
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct Event {
    /// The streamer symbol this event is about.
    ///
    /// Streamer symbols are not always the same string as the instrument
    /// symbol; see `TastyTrade::get_streamer_symbol`.
    pub sym: String,
    /// The event itself.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Eleven kinds, and the wire names have to be exactly what the feed
    /// uses: a misspelling is accepted by the venue and then delivers nothing.
    #[test]
    fn every_event_kind_has_its_wire_name() {
        assert_eq!(EventKind::ALL.len(), 11);

        let names: Vec<&str> = EventKind::ALL.iter().map(EventKind::wire_name).collect();
        assert_eq!(
            names,
            [
                "Quote",
                "TradeETH",
                "Trade",
                "Greeks",
                "Candle",
                "Summary",
                "TimeAndSale",
                "Profile",
                "Underlying",
                "TheoPrice",
                "Series",
            ]
        );

        // Only candles are addressed by a symbol that carries a period.
        for kind in EventKind::ALL {
            assert_eq!(kind.needs_a_period(), kind == EventKind::Candle, "{kind}");
        }
    }

    /// A caller must never build `AAPL{=5m}` by hand, so the rendering is
    /// pinned.
    #[test]
    fn a_period_renders_the_suffix_the_feed_expects() {
        let cases = [
            (CandlePeriod::seconds(15), "{=15s}"),
            (CandlePeriod::minutes(5), "{=5m}"),
            (CandlePeriod::hours(1), "{=1h}"),
            (CandlePeriod::days(1), "{=1d}"),
            (CandlePeriod::weeks(2), "{=2w}"),
            (CandlePeriod::months(3), "{=3mo}"),
        ];

        for (period, expected) in cases {
            let period = period.expect("a positive count is a period");
            assert_eq!(period.suffix(), expected);
            assert_eq!(period.to_string(), expected);
            assert_eq!(period.streamer_symbol("AAPL"), format!("AAPL{expected}"));
        }
    }

    /// A zero-length candle renders a suffix the venue accepts and never
    /// fills, which is indistinguishable from a quiet market.
    ///
    /// The constructors refuse it — and, more to the point, there is no way
    /// around them. An earlier version was an enum with public payloads, so
    /// `CandlePeriod::Minutes(0)` walked straight past the validation; the
    /// count is now a private `NonZeroU32`, which makes zero unrepresentable
    /// rather than merely rejected.
    #[test]
    fn a_zero_period_is_refused_and_cannot_be_built_another_way() {
        for period in [
            CandlePeriod::seconds(0),
            CandlePeriod::minutes(0),
            CandlePeriod::hours(0),
            CandlePeriod::days(0),
            CandlePeriod::weeks(0),
            CandlePeriod::months(0),
        ] {
            let error = period.expect_err("zero is not a period");
            assert!(
                matches!(error, crate::TastyTradeError::Precondition(_)),
                "{error:?}"
            );
        }

        // Every unit, through the general constructor too.
        for unit in [
            CandleUnit::Seconds,
            CandleUnit::Minutes,
            CandleUnit::Hours,
            CandleUnit::Days,
            CandleUnit::Weeks,
            CandleUnit::Months,
        ] {
            assert!(CandlePeriod::new(0, unit).is_err(), "{unit}");
            let period = CandlePeriod::new(1, unit).expect("one is a period");
            assert_eq!(period.count(), 1);
            assert_eq!(period.unit(), unit);
        }

        // A period that exists always renders a non-zero count, so no
        // constructed value can produce `{=0…}`.
        assert!(
            !CandlePeriod::minutes(5)
                .expect("a period")
                .suffix()
                .contains("=0")
        );
    }

    /// Two periods of one underlying are two different streamer symbols. That
    /// is the whole mechanism that keeps them from cross-delivering.
    #[test]
    fn two_periods_of_one_underlying_are_different_symbols() {
        let five = CandlePeriod::minutes(5).expect("a period");
        let hour = CandlePeriod::hours(1).expect("a period");

        assert_ne!(
            five.streamer_symbol("AAPL"),
            hour.streamer_symbol("AAPL"),
            "the period has to be part of the symbol"
        );
    }

    /// A caller routing on the kind must not have to match eleven variants.
    #[test]
    fn event_data_reports_its_own_kind() {
        assert_eq!(
            EventData::Quote(DxfQuoteT::default()).kind(),
            EventKind::Quote
        );
        assert_eq!(
            EventData::Trade(DxfTradeT::default()).kind(),
            EventKind::Trade
        );
        assert_eq!(
            EventData::Greeks(DxfGreeksT::default()).kind(),
            EventKind::Greeks
        );
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
            }
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
            }
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
            }
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
            EventData::Quote(_) => {} // Success
            _ => panic!("Expected Quote variant"),
        }

        match trade_data {
            EventData::Trade(_) => {} // Success
            _ => panic!("Expected Trade variant"),
        }

        match greeks_data {
            EventData::Greeks(_) => {} // Success
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
