//! One snapshot of a price, over REST.
//!
//! Before this, the only way to get a price out of this crate was to open a
//! DXLink websocket, authenticate, open a channel, subscribe and wait. For a
//! caller who wants one look at twenty symbols — a portfolio mark, a screener
//! pass, a pre-trade sanity check — that is the wrong shape entirely.
//!
//! **This is REST money, so every price is `Decimal`.** The `f64` exemption in
//! [`crate::types::dxfeed`] is specifically for the streaming types, where the
//! feed imposes the representation. These are not those types: the field sets
//! differ, and conflating them would leak `f64` onto a REST path.

use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::query::QueryBuilder;
use crate::types::instrument::InstrumentType;

/// The most symbols the venue accepts across all types in one request.
pub const MAX_MARKET_DATA_SYMBOLS: usize = 100;

/// One instrument's prices at a moment in time.
///
/// Every field is `Option<T>`: what arrives depends on the instrument. An
/// equity carries a beta and a dividend, a cryptocurrency does not, and an
/// instrument that has not traded today has no `close`. A price that defaults
/// to zero is a price that lies.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarketDataSnapshot {
    /// The instrument.
    pub symbol: String,
    /// What kind of instrument it is.
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
    /// When the venue last updated it, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub updated_at: Option<DateTime<FixedOffset>>,
    /// Best bid.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub bid: Option<Decimal>,
    /// Size at the bid.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub bid_size: Option<Decimal>,
    /// Best ask.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub ask: Option<Decimal>,
    /// Size at the ask.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub ask_size: Option<Decimal>,
    /// Midpoint of the spread.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub mid: Option<Decimal>,
    /// The venue's mark.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub mark: Option<Decimal>,
    /// Last traded price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub last: Option<Decimal>,
    /// Last price from the primary market.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub last_mkt: Option<Decimal>,
    /// Today's open.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub open: Option<Decimal>,
    /// Today's close, once there is one.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub close: Option<Decimal>,
    /// How the close was determined, e.g. `Final` or `Regular`.
    #[serde(default)]
    pub close_price_type: Option<String>,
    /// The previous close.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub prev_close: Option<Decimal>,
    /// How the previous close was determined.
    #[serde(default)]
    pub prev_close_price_type: Option<String>,
    /// Which day the previous close is from.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub prev_close_date: Option<NaiveDate>,
    /// Which day the summary figures are from.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub summary_date: Option<NaiveDate>,
    /// Today's high.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub day_high_price: Option<Decimal>,
    /// Today's low.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub day_low_price: Option<Decimal>,
    /// The year's high.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub year_high_price: Option<Decimal>,
    /// The year's low.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub year_low_price: Option<Decimal>,
    /// The exchange's lower limit price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub low_limit_price: Option<Decimal>,
    /// The exchange's upper limit price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub high_limit_price: Option<Decimal>,
    /// Volume traded.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub volume: Option<Decimal>,
    /// Beta against the market, for instruments that have one.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub beta: Option<Decimal>,
    /// Dividend amount, for instruments that pay one.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub dividend_amount: Option<Decimal>,
    /// How many times a year the dividend is paid.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub dividend_frequency: Option<Decimal>,
    /// Whether trading is halted. `None` means the venue did not say, which is
    /// not the same as "no".
    #[serde(default)]
    pub is_trading_halted: Option<bool>,
    /// When a halt started. The venue sends `-1` for "not halted", which is a
    /// sentinel rather than a time — left as the integer it is rather than
    /// converted into a timestamp of 1969.
    #[serde(default)]
    pub halt_start_time: Option<i64>,
    /// When a halt ends, with the same `-1` sentinel.
    #[serde(default)]
    pub halt_end_time: Option<i64>,
}

/// Which symbols to fetch, grouped by instrument type.
///
/// The venue takes one query parameter per type, each a **comma-separated**
/// list — `equity=AAPL,TSLA&cryptocurrency=BTC/USD`. Not repeated keys, which
/// is what the instrument listings use; getting it wrong returns one symbol per
/// type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketDataRequest {
    indices: Vec<String>,
    equities: Vec<String>,
    equity_options: Vec<String>,
    futures: Vec<String>,
    future_options: Vec<String>,
    cryptocurrencies: Vec<String>,
}

impl MarketDataRequest {
    /// An empty request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds index symbols.
    #[must_use]
    pub fn with_indices(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.indices, symbols);
        self
    }

    /// Adds equity symbols.
    #[must_use]
    pub fn with_equities(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.equities, symbols);
        self
    }

    /// Adds equity-option symbols, in OCC symbology.
    #[must_use]
    pub fn with_equity_options(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.equity_options, symbols);
        self
    }

    /// Adds futures symbols.
    #[must_use]
    pub fn with_futures(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.futures, symbols);
        self
    }

    /// Adds future-option symbols.
    #[must_use]
    pub fn with_future_options(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.future_options, symbols);
        self
    }

    /// Adds cryptocurrency symbols.
    ///
    /// Market data is unaffected by the venue's order-routing suspension; see
    /// [`crate::prelude::CRYPTOCURRENCY_TRADING_ENABLED`].
    #[must_use]
    pub fn with_cryptocurrencies(mut self, symbols: &[impl AsRef<str>]) -> Self {
        extend(&mut self.cryptocurrencies, symbols);
        self
    }

    /// How many symbols this asks for, across every type.
    pub fn symbol_count(&self) -> usize {
        self.indices.len()
            + self.equities.len()
            + self.equity_options.len()
            + self.futures.len()
            + self.future_options.len()
            + self.cryptocurrencies.len()
    }

    /// Fails when the request asks for more than the venue accepts.
    ///
    /// [`crate::TastyTradeError::Precondition`], so `is_retryable()` is false:
    /// nothing was sent, and sending it again would fail the same way. The
    /// limit belongs here rather than at the venue because a caller building a
    /// watchlist should find out before the round trip.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        // Each entry has to be a symbol before it is counted as one. The wire
        // format joins a group with commas, so a blank entry sends an empty
        // symbol the venue has to interpret, and an entry that *contains* a
        // comma expands into several — which walks straight past the cap this
        // method exists to enforce, since the cap counts entries.
        for (group, symbols) in [
            ("index", &self.indices),
            ("equity", &self.equities),
            ("equity-option", &self.equity_options),
            ("future", &self.futures),
            ("future-option", &self.future_options),
            ("cryptocurrency", &self.cryptocurrencies),
        ] {
            for symbol in symbols {
                if symbol.trim().is_empty() {
                    return Err(crate::TastyTradeError::Precondition(format!(
                        "the {group} list has a blank symbol, which asks the venue for \
                         an instrument with no name"
                    )));
                }
                if symbol.contains(',') {
                    return Err(crate::TastyTradeError::Precondition(format!(
                        "the {group} symbol {symbol:?} contains a comma, which is the \
                         separator this parameter is joined with; pass the symbols \
                         separately so they are counted separately"
                    )));
                }
            }
        }

        let count = self.symbol_count();
        if count > MAX_MARKET_DATA_SYMBOLS {
            return Err(crate::TastyTradeError::Precondition(format!(
                "market data takes at most {MAX_MARKET_DATA_SYMBOLS} symbols across all \
                 instrument types, and this request has {count}; split it into batches"
            )));
        }
        if count == 0 {
            return Err(crate::TastyTradeError::Precondition(
                "a market data request with no symbols asks for nothing".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        // One key per non-empty group, exactly once, comma-joined.
        for (key, symbols) in [
            ("index", &self.indices),
            ("equity", &self.equities),
            ("equity-option", &self.equity_options),
            ("future", &self.futures),
            ("future-option", &self.future_options),
            ("cryptocurrency", &self.cryptocurrencies),
        ] {
            if !symbols.is_empty() {
                query.push(key, symbols.join(","));
            }
        }
        query
    }
}

fn extend(target: &mut Vec<String>, symbols: &[impl AsRef<str>]) {
    target.extend(symbols.iter().map(|symbol| symbol.as_ref().to_owned()));
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../Doc/market_data_by_type.json");

    fn snapshots() -> Vec<MarketDataSnapshot> {
        let body: serde_json::Value = serde_json::from_str(FIXTURE).expect("valid JSON");
        body["data"]["items"]
            .as_array()
            .expect("items")
            .iter()
            .map(|item| serde_json::from_value(item.clone()).expect("a snapshot"))
            .collect()
    }

    #[test]
    fn the_venues_own_payload_decodes_with_prices_as_decimal() {
        let rows = snapshots();
        let bitcoin = &rows[0];

        assert_eq!(bitcoin.symbol, "BTC/USD");
        assert_eq!(
            bitcoin.instrument_type,
            Some(InstrumentType::Cryptocurrency)
        );
        // Every digit the venue sent, which is what `Decimal` is for.
        assert_eq!(bitcoin.bid.expect("a bid").to_string(), "94005.47");
        assert_eq!(bitcoin.mid.expect("a mid").to_string(), "94485.805");
        assert_eq!(
            bitcoin.year_high_price.expect("a high").to_string(),
            "109558.42"
        );
    }

    /// What arrives depends on the instrument, which is why every field is
    /// optional. A cryptocurrency has no beta and no dividend.
    #[test]
    fn a_field_that_does_not_apply_is_absent_rather_than_zero() {
        let rows = snapshots();
        let bitcoin = &rows[0];
        let apple = rows
            .iter()
            .find(|row| row.symbol == "AAPL")
            .expect("the fixture has an equity");

        assert_eq!(bitcoin.beta, None);
        assert_eq!(bitcoin.dividend_amount, None);
        assert_eq!(bitcoin.close, None, "it had not closed yet");

        assert_eq!(apple.beta.expect("a beta").to_string(), "1.260672228");
        assert_eq!(
            apple.dividend_amount.expect("a dividend").to_string(),
            "0.25"
        );
    }

    /// `-1` is the venue's "not halted" sentinel, not a time in 1969.
    #[test]
    fn the_halt_sentinel_is_left_as_the_integer_it_is() {
        let rows = snapshots();

        assert_eq!(rows[0].is_trading_halted, Some(false));
        assert_eq!(rows[0].halt_start_time, Some(-1));
        assert_eq!(rows[0].halt_end_time, Some(-1));
    }

    /// One key per type, sent once, comma-joined. Repeated keys — which is what
    /// the instrument listings use — would return one symbol per type.
    #[test]
    fn each_type_is_one_comma_joined_parameter() {
        let request = MarketDataRequest::new()
            .with_equities(&["AAPL", "TSLA"])
            .with_cryptocurrencies(&["BTC/USD"]);

        assert_eq!(
            request.to_query().pairs(),
            vec![("equity", "AAPL,TSLA"), ("cryptocurrency", "BTC/USD")]
        );
    }

    #[test]
    fn an_empty_group_sends_no_key_at_all() {
        let request = MarketDataRequest::new().with_indices(&["SPX"]);

        assert_eq!(request.to_query().pairs(), vec![("index", "SPX")]);
    }

    #[test]
    fn every_documented_type_is_reachable() {
        let request = MarketDataRequest::new()
            .with_indices(&["SPX"])
            .with_equities(&["AAPL"])
            .with_equity_options(&["SPY   250428P00355000"])
            .with_futures(&["/CLM5"])
            .with_future_options(&["/MESU5EX3M5 250620C6450"])
            .with_cryptocurrencies(&["BTC/USD"]);

        assert_eq!(request.symbol_count(), 6);
        assert_eq!(
            request
                .to_query()
                .pairs()
                .into_iter()
                .map(|(key, _)| key)
                .collect::<Vec<_>>(),
            vec![
                "index",
                "equity",
                "equity-option",
                "future",
                "future-option",
                "cryptocurrency"
            ]
        );
    }

    /// The limit is across **all** types together, which is the part a caller
    /// building a watchlist per type would get wrong.
    #[test]
    fn the_limit_counts_every_type_together() {
        let half: Vec<String> = (0..50).map(|i| format!("SYM{i}")).collect();
        let at_limit = MarketDataRequest::new()
            .with_equities(&half)
            .with_futures(&half);
        assert_eq!(at_limit.symbol_count(), MAX_MARKET_DATA_SYMBOLS);
        assert!(at_limit.validate().is_ok());

        let over = at_limit.with_indices(&["SPX"]);
        let error = over.validate().expect_err("101 symbols is over the limit");
        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable());
        assert!(format!("{error}").contains("100"), "{error}");
    }

    /// Asking for nothing is a caller mistake, and the venue's answer to it
    /// would be an empty listing that looks like "no data".
    #[test]
    fn an_empty_request_is_refused() {
        assert!(MarketDataRequest::new().validate().is_err());
    }

    /// An entry is not a symbol just because it is there.
    ///
    /// The wire format joins each group with commas, so a blank entry asks for
    /// an instrument with no name and an entry that carries a comma expands
    /// into several — which walks past the 100-symbol cap, because the cap
    /// counts entries. Both used to satisfy the non-empty check.
    #[test]
    fn a_blank_or_comma_bearing_entry_is_not_a_symbol() {
        let blank = MarketDataRequest::new().with_equities(&["   "]);
        let error = blank
            .validate()
            .expect_err("whitespace is not an instrument");
        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable(), "nothing was sent");

        assert!(
            MarketDataRequest::new()
                .with_equities(&["AAPL,MSFT"])
                .validate()
                .is_err(),
            "one entry must not expand into two symbols"
        );

        // Every group is checked, not only the first.
        for request in [
            MarketDataRequest::new().with_indices(&[""]),
            MarketDataRequest::new().with_equity_options(&["\t"]),
            MarketDataRequest::new().with_futures(&["/ES,/NQ"]),
            MarketDataRequest::new().with_future_options(&[" "]),
            MarketDataRequest::new().with_cryptocurrencies(&["BTC/USD,ETH/USD"]),
        ] {
            assert!(request.validate().is_err(), "a group went unchecked");
        }

        // And a real request is untouched.
        MarketDataRequest::new()
            .with_equities(&["AAPL", "MSFT"])
            .validate()
            .expect("ordinary symbols must be accepted");
    }
}
