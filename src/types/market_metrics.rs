//! Implied volatility, liquidity, dividends and earnings.
//!
//! For an options client this is the data that decides whether a trade is worth
//! putting on at all: the crate could fetch an option chain but not tell you
//! whether its volatility was high or low.
//!
//! **REST fields, so `Decimal`.** IV and greeks also exist in
//! [`crate::types::dxfeed`] as `f64`, but that exemption is specifically for
//! the streaming types where the feed imposes the representation. These are not
//! those.

use chrono::NaiveDate;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::query::QueryBuilder;

/// Volatility and liquidity for one underlying.
///
/// Every field is `Option<T>`: the venue sends what it has, and an instrument
/// with no options history has no IV rank. A rank that defaults to zero reads
/// as "cheapest volatility all year", which is the opposite of unknown.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarketMetric {
    /// The underlying.
    pub symbol: String,
    /// Current implied-volatility index.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub implied_volatility_index: Option<Decimal>,
    /// How much that index moved over five days.
    #[serde(
        rename = "implied-volatility-index-5-day-change",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub implied_volatility_index_5_day_change: Option<Decimal>,
    /// Where the current IV sits in its own yearly range, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub implied_volatility_rank: Option<Decimal>,
    /// What fraction of the year IV was below where it is now.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub implied_volatility_percentile: Option<Decimal>,
    /// Raw liquidity measure.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub liquidity: Option<Decimal>,
    /// Where that liquidity sits in its own range.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub liquidity_rank: Option<Decimal>,
    /// The venue's liquidity rating, an integer bucket rather than a measure.
    #[serde(default)]
    pub liquidity_rating: Option<i64>,
    /// One entry per option expiration.
    #[serde(default)]
    pub option_expiration_implied_volatilities: Vec<ExpirationImpliedVolatility>,
}

/// Implied volatility for one option expiration.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ExpirationImpliedVolatility {
    /// Which expiration.
    ///
    /// A **calendar day**, per the rule the rest of this crate follows: an
    /// expiration is a day of market, and there is no timezone to invent. The
    /// venue's schema types it `date-time`, so both shapes decode and the day
    /// is what is kept.
    #[serde(default, with = "crate::types::wire::expiration_date_option")]
    pub expiration_date: Option<NaiveDate>,
    /// Whether the series settles in the morning or the afternoon.
    #[serde(default)]
    pub settlement_type: Option<String>,
    /// Standard or non-standard.
    ///
    /// Left as `String` on purpose. `CLAUDE.md` records that
    /// `option_chain_type` has no captured value set, and guessing the
    /// variants from the field name produces variants that never match.
    #[serde(default)]
    pub option_chain_type: Option<String>,
    /// Implied volatility for the expiration.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub implied_volatility: Option<Decimal>,
}

/// One historical dividend.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct DividendReport {
    /// When it was paid.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub occurred_date: Option<NaiveDate>,
    /// Per-share amount.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub amount: Option<Decimal>,
}

/// One historical earnings announcement.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct EarningsReport {
    /// When it was announced.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub occurred_date: Option<NaiveDate>,
    /// Earnings per share.
    ///
    /// `Decimal`, and it can be negative — a loss is a real answer, not a
    /// missing one.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub eps: Option<Decimal>,
}

/// Which earnings announcements to fetch.
///
/// `start_date` is a required argument rather than an `Option`, because the
/// venue marks it required: a missing required query parameter should be
/// impossible to express, not a runtime `400`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarningsRange {
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
}

impl EarningsRange {
    /// From `start_date` onwards.
    pub fn from(start_date: NaiveDate) -> Self {
        Self {
            start_date,
            end_date: None,
        }
    }

    /// A closed range.
    pub fn between(start_date: NaiveDate, end_date: NaiveDate) -> Self {
        Self {
            start_date,
            end_date: Some(end_date),
        }
    }

    /// The first day covered.
    pub fn start_date(&self) -> NaiveDate {
        self.start_date
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push("start-date", self.start_date);
        query.push_opt("end-date", self.end_date);
        query
    }
}

/// Builds the `symbols` parameter for [`crate::TastyTrade::market_metrics`].
///
/// **Comma-joined into one parameter**, not repeated keys. The venue documents
/// it that way and getting it wrong returns metrics for one symbol — which
/// looks like a thin answer rather than a client bug.
pub(crate) fn symbols_query(symbols: &[impl AsRef<str>]) -> QueryBuilder {
    let mut query = QueryBuilder::new();
    query.push(
        "symbols",
        symbols
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join(","),
    );
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    /// The trap this endpoint sets: `symbols` is one comma-joined parameter,
    /// unlike the repeated `symbol[]` keys everywhere else in the crate.
    #[test]
    fn symbols_are_comma_joined_into_one_parameter() {
        let query = symbols_query(&["AAPL", "TSLA", "SPY"]);

        assert_eq!(query.pairs(), vec![("symbols", "AAPL,TSLA,SPY")]);
    }

    #[test]
    fn a_single_symbol_still_uses_the_same_parameter() {
        assert_eq!(symbols_query(&["AAPL"]).pairs(), vec![("symbols", "AAPL")]);
    }

    /// `start-date` is required by the venue and required by the type, so the
    /// only question is whether `end-date` was given.
    #[test]
    fn an_earnings_range_always_carries_its_start_date() {
        let open = EarningsRange::from(day(2026, 1, 1));
        assert_eq!(open.to_query().pairs(), vec![("start-date", "2026-01-01")]);

        let closed = EarningsRange::between(day(2026, 1, 1), day(2026, 3, 31));
        assert_eq!(
            closed.to_query().pairs(),
            vec![("start-date", "2026-01-01"), ("end-date", "2026-03-31")]
        );
    }

    /// The expiration is a calendar day whichever shape the venue sends.
    #[test]
    fn an_expiration_decodes_from_either_shape_and_keeps_the_day() {
        let plain: ExpirationImpliedVolatility =
            serde_json::from_str(r#"{"expiration-date": "2026-05-15"}"#)
                .expect("a plain date decodes");
        assert_eq!(plain.expiration_date, Some(day(2026, 5, 15)));

        let stamped: ExpirationImpliedVolatility =
            serde_json::from_str(r#"{"expiration-date": "2026-05-15T00:00:00.000-04:00"}"#)
                .expect("a timestamp decodes");
        assert_eq!(
            stamped.expiration_date,
            Some(day(2026, 5, 15)),
            "the day is what an expiration means"
        );
    }

    /// Anything that is neither is still an error: a helper that swallowed
    /// every bad value would turn a decoding bug into a missing expiration.
    #[test]
    fn a_malformed_expiration_is_an_error() {
        assert!(
            serde_json::from_str::<ExpirationImpliedVolatility>(
                r#"{"expiration-date": "next tuesday"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn a_metric_decodes_with_its_volatilities_as_decimal() {
        let metric: MarketMetric = serde_json::from_str(
            r#"{"symbol": "AAPL",
                "implied-volatility-index": "0.3421",
                "implied-volatility-index-5-day-change": "-0.0123",
                "implied-volatility-rank": "0.5117",
                "liquidity-rating": 4,
                "option-expiration-implied-volatilities": [
                    {"expiration-date": "2026-05-15", "settlement-type": "PM",
                     "option-chain-type": "Standard", "implied-volatility": "0.29"}
                ]}"#,
        )
        .expect("the metric must decode");

        assert_eq!(
            metric
                .implied_volatility_index
                .expect("an index")
                .to_string(),
            "0.3421"
        );
        // A negative change is a real answer.
        assert_eq!(
            metric
                .implied_volatility_index_5_day_change
                .expect("a change")
                .to_string(),
            "-0.0123"
        );
        assert_eq!(metric.liquidity_rating, Some(4));
        assert_eq!(metric.option_expiration_implied_volatilities.len(), 1);
        // …and what the venue did not send stays absent. A rank of zero would
        // read as "cheapest volatility all year".
        assert_eq!(metric.implied_volatility_percentile, None);
        assert_eq!(metric.liquidity, None);
    }

    /// A loss is a real earnings figure, not a missing one.
    #[test]
    fn negative_earnings_per_share_decodes() {
        let report: EarningsReport =
            serde_json::from_str(r#"{"occurred-date": "2026-02-01", "eps": "-1.25"}"#)
                .expect("the report must decode");

        assert_eq!(report.eps.expect("an eps").to_string(), "-1.25");
        assert_eq!(report.occurred_date, Some(day(2026, 2, 1)));
    }

    #[test]
    fn a_dividend_decodes() {
        let dividend: DividendReport =
            serde_json::from_str(r#"{"occurred-date": "2026-02-10", "amount": 0.25}"#)
                .expect("the dividend must decode");

        assert_eq!(dividend.amount.expect("an amount").to_string(), "0.25");
    }
}
