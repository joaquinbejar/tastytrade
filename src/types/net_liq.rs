//! The account's equity curve.
//!
//! Open, high, low and close of net liquidating value over time, which is what
//! a performance or drawdown chart is drawn from.
//!
//! Two things about this endpoint are unlike the rest of the API. It is served
//! by a different system — the swagger is OpenAPI 3 from a JVM service, where
//! everything else is Swagger 2 — and that system spells its JSON in
//! **camelCase** rather than the kebab-case every other tastytrade response
//! uses. And per the venue's sandbox page it is **live only**: certification
//! does not serve it.

use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::query::QueryBuilder;

/// How far back to ask for, relative to now.
///
/// A closed enum: the venue's schema enumerates exactly these seven values, so
/// an eighth is a change to the contract rather than something to round-trip.
/// This is a **request** type — unlike the response-side `wire_enum!`s, where an
/// unrecognised value must survive because dropping it loses data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBack {
    /// One day.
    OneDay,
    /// One week.
    OneWeek,
    /// One month.
    OneMonth,
    /// Three months.
    ThreeMonths,
    /// Six months.
    SixMonths,
    /// One year.
    OneYear,
    /// Everything the venue holds.
    All,
}

impl TimeBack {
    /// The text the venue uses.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::OneDay => "1d",
            Self::OneWeek => "1w",
            Self::OneMonth => "1m",
            Self::ThreeMonths => "3m",
            Self::SixMonths => "6m",
            Self::OneYear => "1y",
            Self::All => "all",
        }
    }
}

impl std::fmt::Display for TimeBack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// Which stretch of history to ask for.
///
/// `time-back` and an explicit window are alternatives at the venue, and a
/// request carrying both is one it has to resolve however it likes. One enum
/// makes that unrepresentable rather than documented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetLiqRange {
    /// Relative to now.
    Back(TimeBack),
    /// An explicit window.
    ///
    /// The venue documents the format as
    /// `2011-12-03T10:15:30+01:00[Europe/Paris]` — a JVM `ZonedDateTime`, with
    /// a bracketed zone identifier that RFC 3339 does not have and `chrono`
    /// does not produce. So these are `String`: handing over a
    /// `DateTime<FixedOffset>` would render something the venue did not ask
    /// for, and inventing a zone name is worse.
    Window {
        /// Start of the window, in the venue's format.
        start_time: Option<String>,
        /// End of the window, in the venue's format.
        end_time: Option<String>,
    },
}

impl NetLiqRange {
    /// Relative to now.
    pub fn back(time_back: TimeBack) -> Self {
        Self::Back(time_back)
    }

    /// An explicit window, in the venue's `ZonedDateTime` format.
    pub fn window(start_time: impl Into<String>, end_time: impl Into<String>) -> Self {
        Self::Window {
            start_time: Some(start_time.into()),
            end_time: Some(end_time.into()),
        }
    }

    /// From an instant onwards.
    pub fn from(start_time: impl Into<String>) -> Self {
        Self::Window {
            start_time: Some(start_time.into()),
            end_time: None,
        }
    }

    fn write_into(&self, query: &mut QueryBuilder) {
        match self {
            Self::Back(time_back) => query.push("time-back", time_back.as_wire()),
            Self::Window {
                start_time,
                end_time,
            } => {
                query.push_opt("start-time", start_time.as_ref());
                query.push_opt("end-time", end_time.as_ref());
            }
        }
    }
}

impl Default for NetLiqRange {
    /// Whatever the venue considers recent: neither end bounded.
    fn default() -> Self {
        Self::Window {
            start_time: None,
            end_time: None,
        }
    }
}

/// One bar of the account's equity curve.
///
/// The **camelCase** field names are this service's own; every other tastytrade
/// response is kebab-case. Each field also accepts the kebab-case spelling as
/// an alias — the two systems have disagreed before, and a listing that decodes
/// into a row of `None` because the case changed is a silently empty chart
/// rather than an error.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct NetLiqOhlc {
    /// Net liquidating value at the start of the bar.
    #[serde(default, alias = "open", with = "crate::types::wire::decimal_option")]
    pub open: Option<Decimal>,
    /// Highest net liquidating value in the bar.
    #[serde(default, alias = "high", with = "crate::types::wire::decimal_option")]
    pub high: Option<Decimal>,
    /// Lowest net liquidating value in the bar.
    #[serde(default, alias = "low", with = "crate::types::wire::decimal_option")]
    pub low: Option<Decimal>,
    /// Net liquidating value at the end of the bar.
    #[serde(default, alias = "close", with = "crate::types::wire::decimal_option")]
    pub close: Option<Decimal>,
    /// Total value at the start of the bar.
    #[serde(
        rename = "totalOpen",
        alias = "total-open",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub total_open: Option<Decimal>,
    /// Highest total value in the bar.
    #[serde(
        rename = "totalHigh",
        alias = "total-high",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub total_high: Option<Decimal>,
    /// Lowest total value in the bar.
    #[serde(
        rename = "totalLow",
        alias = "total-low",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub total_low: Option<Decimal>,
    /// Total value at the end of the bar.
    #[serde(
        rename = "totalClose",
        alias = "total-close",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub total_close: Option<Decimal>,
    /// Pending cash at the start of the bar.
    #[serde(
        rename = "pendingCashOpen",
        alias = "pending-cash-open",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub pending_cash_open: Option<Decimal>,
    /// Highest pending cash in the bar.
    #[serde(
        rename = "pendingCashHigh",
        alias = "pending-cash-high",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub pending_cash_high: Option<Decimal>,
    /// Lowest pending cash in the bar.
    #[serde(
        rename = "pendingCashLow",
        alias = "pending-cash-low",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub pending_cash_low: Option<Decimal>,
    /// Pending cash at the end of the bar.
    #[serde(
        rename = "pendingCashClose",
        alias = "pending-cash-close",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub pending_cash_close: Option<Decimal>,
    /// When the bar is for, exactly as the venue sent it.
    ///
    /// **Not** a `DateTime`. The schema marks this a plain string with no
    /// `format`, and the same service documents its input timestamps as JVM
    /// `ZonedDateTime` — `2011-12-03T10:15:30+01:00[Europe/Paris]` — which is
    /// not RFC 3339. Assigning a stronger type on that evidence would make
    /// every bar fail to decode the first time the guess was wrong.
    #[serde(default)]
    pub time: Option<String>,
}

/// Which stretch of the equity curve to fetch, and at what resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetLiqHistoryFilter {
    range: NetLiqRange,
    interval: Option<String>,
}

impl NetLiqHistoryFilter {
    /// Whatever the venue considers recent, at its own resolution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Relative to now.
    pub fn back(time_back: TimeBack) -> Self {
        Self::new().with_range(NetLiqRange::back(time_back))
    }

    /// Which stretch of history.
    #[must_use]
    pub fn with_range(mut self, range: NetLiqRange) -> Self {
        self.range = range;
        self
    }

    /// How coarse the bars should be.
    ///
    /// A `String` rather than an enum: the schema declares `interval` a string
    /// with **no** enumerated values, unlike `time-back` beside it, so there is
    /// no set to close over. It becomes an enum when a payload or a document
    /// shows one.
    #[must_use]
    pub fn with_interval(mut self, interval: impl Into<String>) -> Self {
        self.interval = Some(interval.into());
        self
    }

    /// Which stretch this filter asks for.
    pub fn range(&self) -> &NetLiqRange {
        &self.range
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        self.range.write_into(&mut query);
        query.push_opt("interval", self.interval.as_ref());
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two ranges are alternatives, so only one set of keys can ever be
    /// produced. This is the contradiction the enum removes.
    #[test]
    fn a_relative_range_and_a_window_can_never_be_sent_together() {
        let relative = NetLiqHistoryFilter::back(TimeBack::ThreeMonths);
        assert_eq!(relative.to_query().pairs(), vec![("time-back", "3m")]);

        let windowed = NetLiqHistoryFilter::new().with_range(NetLiqRange::window(
            "2026-01-01T00:00:00+00:00[UTC]",
            "2026-02-01T00:00:00+00:00[UTC]",
        ));
        let query = windowed.to_query();
        let pairs = query.pairs();
        assert!(pairs.iter().all(|(key, _)| *key != "time-back"));
        assert_eq!(
            pairs,
            vec![
                ("start-time", "2026-01-01T00:00:00+00:00[UTC]"),
                ("end-time", "2026-02-01T00:00:00+00:00[UTC]"),
            ]
        );
    }

    #[test]
    fn every_documented_time_back_value_has_its_own_spelling() {
        for (value, wire) in [
            (TimeBack::OneDay, "1d"),
            (TimeBack::OneWeek, "1w"),
            (TimeBack::OneMonth, "1m"),
            (TimeBack::ThreeMonths, "3m"),
            (TimeBack::SixMonths, "6m"),
            (TimeBack::OneYear, "1y"),
            (TimeBack::All, "all"),
        ] {
            assert_eq!(value.as_wire(), wire);
            assert_eq!(value.to_string(), wire);
        }
    }

    #[test]
    fn an_unfiltered_request_sends_nothing() {
        assert!(NetLiqHistoryFilter::new().to_query().pairs().is_empty());
    }

    #[test]
    fn the_interval_is_sent_alongside_either_range() {
        let filter = NetLiqHistoryFilter::back(TimeBack::All).with_interval("1d");

        assert_eq!(
            filter.to_query().pairs(),
            vec![("time-back", "all"), ("interval", "1d")]
        );
    }

    /// This service spells its JSON in camelCase, unlike every other
    /// tastytrade response.
    #[test]
    fn a_bar_decodes_from_the_camel_case_the_service_documents() {
        let bar: NetLiqOhlc = serde_json::from_str(
            r#"{"open": 1000.5, "high": 1100.25, "low": 990.0, "close": 1050.75,
                "totalOpen": 2000.5, "totalClose": 2050.75,
                "pendingCashOpen": 0.0, "pendingCashClose": 25.0,
                "time": "2026-08-03T00:00:00Z"}"#,
        )
        .expect("the bar must decode");

        assert_eq!(bar.open.expect("an open").to_string(), "1000.5");
        assert_eq!(
            bar.total_close.expect("a total close").to_string(),
            "2050.75"
        );
        assert_eq!(
            bar.pending_cash_close.expect("pending cash").to_string(),
            "25.0"
        );
        // `time` stays exactly what arrived.
        assert_eq!(bar.time.as_deref(), Some("2026-08-03T00:00:00Z"));
    }

    /// …and the kebab-case alias, because the two systems have disagreed
    /// before and a chart that silently comes back empty is worse than one
    /// that fails.
    #[test]
    fn a_bar_also_decodes_from_kebab_case() {
        let bar: NetLiqOhlc = serde_json::from_str(
            r#"{"open": "1000.5", "total-close": "2050.75", "pending-cash-close": "25.0"}"#,
        )
        .expect("the bar must decode either way");

        assert_eq!(
            bar.total_close.expect("a total close").to_string(),
            "2050.75"
        );
        assert_eq!(
            bar.pending_cash_close.expect("pending cash").to_string(),
            "25.0"
        );
    }

    /// A bar the venue sent nothing for is `None` throughout rather than a row
    /// of zeros, which on an equity curve would be a drawdown to nothing.
    #[test]
    fn an_empty_bar_is_absent_rather_than_zero() {
        let bar: NetLiqOhlc = serde_json::from_str("{}").expect("an empty bar decodes");

        assert_eq!(bar.open, None);
        assert_eq!(bar.close, None);
        assert_eq!(bar.time, None);
    }
}
