//! When the market is open, and when it is not.
//!
//! Any scheduling built on this library previously had to hardcode a calendar,
//! and hardcoded exchange calendars are wrong roughly once a quarter.
//!
//! Session boundaries are **instants with an offset**, kept as
//! [`chrono::DateTime<FixedOffset>`] with the offset preserved — going to UTC
//! is one-way, and a caller showing a market open in local exchange time needs
//! what the venue sent. Holidays are calendar days, so [`chrono::NaiveDate`].

use std::fmt;

use chrono::{DateTime, FixedOffset, Months, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::api::query::QueryBuilder;
use crate::types::wire::wire_enum;

wire_enum! {
    /// Which set of instruments a session covers, **as the venue reports it**.
    ///
    /// A response value, so it keeps the `Unknown` arm: `Items<T>` drops a row
    /// it cannot decode, and a collection the venue adds later would make
    /// sessions disappear rather than arrive with an unfamiliar name.
    ///
    /// It is deliberately *not* what the request types take. Tolerance on the
    /// way out would let a typo through to a 404, and the published contract
    /// closes every one of these parameters — see [`SessionCollection`] and
    /// [`FuturesExchange`].
    InstrumentCollection {
        Cfe => "CFE",
        Cme => "CME",
        Equity => "Equity",
    }
}

/// A collection to **ask** about, closed to what the contract admits.
///
/// `instrument-collections[]` on `GET /market-time/sessions/current` enumerates
/// exactly these three. A tolerant value here would be a typo travelling to a
/// 404, which is the failure the type exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionCollection {
    /// Cboe Futures Exchange.
    Cfe,
    /// CME Group.
    Cme,
    /// Equities.
    Equity,
}

impl SessionCollection {
    /// The spelling the venue expects.
    pub fn as_wire(&self) -> &'static str {
        match self {
            SessionCollection::Cfe => "CFE",
            SessionCollection::Cme => "CME",
            SessionCollection::Equity => "Equity",
        }
    }
}

impl fmt::Display for SessionCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

impl From<FuturesExchange> for SessionCollection {
    fn from(exchange: FuturesExchange) -> Self {
        match exchange {
            FuturesExchange::Cfe => SessionCollection::Cfe,
            FuturesExchange::Cme => SessionCollection::Cme,
        }
    }
}

/// A futures exchange, which is the subset the futures routes admit.
///
/// The four `/market-time/futures/…/{instrument_collection}` operations
/// document their path parameter as "one of: CFE, CME". Passing `Equity` to one
/// of them was representable and produced a 404 after an authenticated round
/// trip; there is no value of this type that can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuturesExchange {
    /// Cboe Futures Exchange.
    Cfe,
    /// CME Group.
    Cme,
}

impl FuturesExchange {
    /// The spelling the venue expects.
    pub fn as_wire(&self) -> &'static str {
        match self {
            FuturesExchange::Cfe => "CFE",
            FuturesExchange::Cme => "CME",
        }
    }
}

impl fmt::Display for FuturesExchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// The longest range `GET /market-time/sessions` accepts.
///
/// Nine months, per the venue. Enforced locally so an over-long request fails
/// in the caller's process rather than after a round trip — and the error names
/// the limit, so the refusal is obviously this crate's rule.
///
/// **Calendar months**, counted from the start of the range rather than
/// approximated in days. `9 * 31` accepted January 1 to October 5, which is
/// longer than nine months by any reading, and rejected ranges the venue would
/// have served. Nine months from a day is what
/// [`chrono::Months`] says it is.
pub const MAX_SESSION_RANGE_MONTHS: u32 = 9;

/// One trading session's boundaries.
///
/// Every timestamp keeps the offset the venue sent. `session_date` is present
/// on the next/previous lookups and absent from a range listing, which is why
/// it is optional rather than two nearly identical types.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarketSession {
    /// Which instruments the session covers.
    #[serde(default)]
    pub instrument_collection: Option<InstrumentCollection>,
    /// The trading day, on the lookups that report one.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub session_date: Option<NaiveDate>,
    /// When the session begins, including pre-market.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub start_at: Option<DateTime<FixedOffset>>,
    /// When regular trading opens.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub open_at: Option<DateTime<FixedOffset>>,
    /// When regular trading closes.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub close_at: Option<DateTime<FixedOffset>>,
    /// When extended trading closes.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub close_at_ext: Option<DateTime<FixedOffset>>,
}

/// The session in progress, with the ones either side of it.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct CurrentMarketSession {
    /// Which instruments the session covers.
    #[serde(default)]
    pub instrument_collection: Option<InstrumentCollection>,
    /// What the venue calls the current state, e.g. `Open` or `Closed`.
    ///
    /// Left as `String`: the schema enumerates no values for it, and guessing
    /// the set from the field name produces variants that never match.
    #[serde(default)]
    pub state: Option<String>,
    /// When the session begins, including pre-market.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub start_at: Option<DateTime<FixedOffset>>,
    /// When regular trading opens.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub open_at: Option<DateTime<FixedOffset>>,
    /// When regular trading closes.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub close_at: Option<DateTime<FixedOffset>>,
    /// When extended trading closes.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub close_at_ext: Option<DateTime<FixedOffset>>,
    /// The session after this one.
    #[serde(default)]
    pub next_session: Option<Box<MarketSession>>,
    /// The session before this one.
    #[serde(default)]
    pub previous_session: Option<Box<MarketSession>>,
}

impl CurrentMarketSession {
    /// Whether regular trading is open at `now`.
    ///
    /// Derived from the session the venue sent, never from a local assumption
    /// about the exchange's timezone — that is the whole reason this endpoint
    /// exists. `None` when the venue did not send both boundaries: "we were not
    /// told" is not "closed".
    ///
    /// `now` is an argument rather than read from the clock so this stays a
    /// pure function and a caller can ask about a moment other than this one.
    pub fn is_open_at(&self, now: DateTime<FixedOffset>) -> Option<bool> {
        let open_at = self.open_at?;
        let close_at = self.close_at?;
        Some(now >= open_at && now < close_at)
    }

    /// Whether extended trading is open at `now`, on the same terms.
    pub fn is_extended_open_at(&self, now: DateTime<FixedOffset>) -> Option<bool> {
        let start_at = self.start_at?;
        let close_at_ext = self.close_at_ext?;
        Some(now >= start_at && now < close_at_ext)
    }
}

/// The days a market is closed or closes early.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct MarketCalendar {
    /// Days the market does not open.
    ///
    /// The venue's schema types this `object` with no properties, which is a
    /// generation artifact — the same document types a decimal quantity that
    /// way. It is an array of calendar days, and that is how it is read.
    #[serde(default)]
    pub market_holidays: Vec<NaiveDate>,
    /// Days the market closes early.
    #[serde(default)]
    pub market_half_days: Vec<NaiveDate>,
}

impl MarketCalendar {
    /// Whether `date` is a full closure.
    pub fn is_holiday(&self, date: NaiveDate) -> bool {
        self.market_holidays.contains(&date)
    }

    /// Whether `date` closes early.
    pub fn is_half_day(&self, date: NaiveDate) -> bool {
        self.market_half_days.contains(&date)
    }
}

/// Which stretch of sessions to fetch from `GET /market-time/sessions`.
///
/// `to_date` is a constructor argument rather than an optional field because
/// the venue marks it required: a missing required query parameter should be
/// impossible to express, not a runtime `400`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRange {
    to_date: NaiveDate,
    from_date: Option<NaiveDate>,
    instrument_collection: Option<SessionCollection>,
}

impl SessionRange {
    /// Sessions up to and including `to_date`.
    pub fn until(to_date: NaiveDate) -> Self {
        Self {
            to_date,
            from_date: None,
            instrument_collection: None,
        }
    }

    /// Sessions between two days.
    pub fn between(from_date: NaiveDate, to_date: NaiveDate) -> Self {
        Self {
            to_date,
            from_date: Some(from_date),
            instrument_collection: None,
        }
    }

    /// Restricts to one instrument collection.
    #[must_use]
    pub fn with_instrument_collection(mut self, collection: SessionCollection) -> Self {
        self.instrument_collection = Some(collection);
        self
    }

    /// The last day covered.
    pub fn to_date(&self) -> NaiveDate {
        self.to_date
    }

    /// Fails when the range is longer than the venue accepts, or inverted.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        let Some(from_date) = self.from_date else {
            // Only `to-date` is required, and with no lower bound there is no
            // span to measure. The venue decides how far back it goes.
            return Ok(());
        };

        if from_date > self.to_date {
            return Err(crate::TastyTradeError::Precondition(format!(
                "the session range starts on {from_date} and ends on {}, which is \
                 before it",
                self.to_date
            )));
        }

        // `checked_add_months` clamps to the last day of the target month, so
        // the boundary from January 31 is October 31 rather than a day that
        // does not exist. `None` means the date is at the end of chrono's
        // range, where nothing can be nine months later.
        let limit = from_date
            .checked_add_months(Months::new(MAX_SESSION_RANGE_MONTHS))
            .ok_or_else(|| {
                crate::TastyTradeError::Precondition(format!(
                    "{from_date} is too far in the future to add nine months to"
                ))
            })?;
        if self.to_date > limit {
            return Err(crate::TastyTradeError::Precondition(format!(
                "the venue answers at most {MAX_SESSION_RANGE_MONTHS} months of sessions, \
                 so from {from_date} the last day it will serve is {limit} and this range \
                 ends {}; split it into shorter ranges",
                self.to_date
            )));
        }

        Ok(())
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push("to-date", self.to_date);
        query.push_opt("from-date", self.from_date);
        query.push_opt(
            "instrument-collection",
            self.instrument_collection
                .as_ref()
                .map(SessionCollection::as_wire),
        );
        query
    }
}

/// Builds the repeated `instrument-collections[]` selection.
///
/// Required by the venue, so the caller passes a first collection and any
/// others separately: an empty selection is unrepresentable rather than a
/// runtime `400`.
pub(crate) fn collections_query(
    first: &SessionCollection,
    rest: &[SessionCollection],
) -> QueryBuilder {
    let mut query = QueryBuilder::new();
    query.push_each(
        "instrument-collections[]",
        std::iter::once(first)
            .chain(rest.iter())
            .map(|collection| collection.as_wire().to_string()),
    );
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    fn moment(text: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(text).expect("an RFC 3339 timestamp")
    }

    /// The rule the whole crate follows: offsets are preserved, because going
    /// to UTC is one-way and a market open shown in the wrong zone is wrong.
    #[test]
    fn a_session_keeps_the_offset_the_venue_sent() {
        let session: MarketSession = serde_json::from_str(
            r#"{"instrument-collection": "Equity", "session-date": "2026-08-03",
                "start-at": "2026-08-03T08:00:00.000-04:00",
                "open-at": "2026-08-03T09:30:00.000-04:00",
                "close-at": "2026-08-03T16:00:00.000-04:00",
                "close-at-ext": "2026-08-03T20:00:00.000-04:00"}"#,
        )
        .expect("the session must decode");

        assert_eq!(
            session.open_at.expect("an open").offset().local_minus_utc(),
            -4 * 3600,
            "the offset must survive rather than being normalised to UTC"
        );
        assert_eq!(session.session_date, Some(day(2026, 8, 3)));
        assert_eq!(
            session.instrument_collection,
            Some(InstrumentCollection::Equity)
        );
    }

    /// The predicate is derived from what the venue sent, never from a local
    /// guess about the exchange's timezone.
    #[test]
    fn openness_is_derived_from_the_fetched_session() {
        let session: CurrentMarketSession = serde_json::from_str(
            r#"{"state": "Open",
                "start-at": "2026-08-03T08:00:00.000-04:00",
                "open-at": "2026-08-03T09:30:00.000-04:00",
                "close-at": "2026-08-03T16:00:00.000-04:00",
                "close-at-ext": "2026-08-03T20:00:00.000-04:00"}"#,
        )
        .expect("the session must decode");

        assert_eq!(
            session.is_open_at(moment("2026-08-03T10:00:00-04:00")),
            Some(true)
        );
        assert_eq!(
            session.is_open_at(moment("2026-08-03T08:30:00-04:00")),
            Some(false),
            "pre-market is not regular trading"
        );
        assert_eq!(
            session.is_extended_open_at(moment("2026-08-03T08:30:00-04:00")),
            Some(true)
        );
        // Comparing across zones has to work, since a caller's clock is
        // wherever they are.
        assert_eq!(
            session.is_open_at(moment("2026-08-03T15:00:00+01:00")),
            Some(true),
            "14:00 UTC is 10:00 in New York"
        );
    }

    /// "We were not told" is not "closed".
    #[test]
    fn a_session_with_no_boundaries_answers_none() {
        let session: CurrentMarketSession =
            serde_json::from_str(r#"{"state": "Closed"}"#).expect("a thin session decodes");

        assert_eq!(
            session.is_open_at(moment("2026-08-03T10:00:00-04:00")),
            None
        );
    }

    #[test]
    fn a_current_session_carries_the_ones_either_side() {
        let session: CurrentMarketSession = serde_json::from_str(
            r#"{"state": "Closed",
                "next-session": {"session-date": "2026-08-04"},
                "previous-session": {"session-date": "2026-07-31"}}"#,
        )
        .expect("the session must decode");

        assert_eq!(
            session.next_session.expect("a next session").session_date,
            Some(day(2026, 8, 4))
        );
        assert_eq!(
            session
                .previous_session
                .expect("a previous session")
                .session_date,
            Some(day(2026, 7, 31))
        );
    }

    #[test]
    fn a_calendar_answers_holidays_and_half_days() {
        let calendar: MarketCalendar = serde_json::from_str(
            r#"{"market-holidays": ["2026-01-01", "2026-07-03"],
                "market-half-days": ["2026-11-27"]}"#,
        )
        .expect("the calendar must decode");

        assert!(calendar.is_holiday(day(2026, 1, 1)));
        assert!(!calendar.is_holiday(day(2026, 11, 27)));
        assert!(calendar.is_half_day(day(2026, 11, 27)));
        assert!(!calendar.is_half_day(day(2026, 1, 1)));
    }

    /// `to-date` is required, and the type makes it impossible to omit.
    #[test]
    fn a_range_always_sends_its_end() {
        let open = SessionRange::until(day(2026, 8, 31));
        assert_eq!(open.to_query().pairs(), vec![("to-date", "2026-08-31")]);

        let closed = SessionRange::between(day(2026, 8, 1), day(2026, 8, 31))
            .with_instrument_collection(SessionCollection::Cme);
        assert_eq!(
            closed.to_query().pairs(),
            vec![
                ("to-date", "2026-08-31"),
                ("from-date", "2026-08-01"),
                ("instrument-collection", "CME"),
            ]
        );
    }

    #[test]
    fn a_range_longer_than_nine_months_is_refused_locally() {
        let error = SessionRange::between(day(2026, 1, 1), day(2027, 1, 1))
            .validate()
            .expect_err("a year is more than nine months");

        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable());
        assert!(format!("{error}").contains("months of sessions"), "{error}");

        assert!(
            SessionRange::between(day(2026, 1, 1), day(2026, 9, 1))
                .validate()
                .is_ok(),
            "eight months is inside the limit"
        );
    }

    /// An inverted range is a caller mistake worth catching, and the venue's
    /// answer to it would be an empty listing that reads as "no sessions".
    #[test]
    fn an_inverted_range_is_refused() {
        assert!(
            SessionRange::between(day(2026, 8, 31), day(2026, 8, 1))
                .validate()
                .is_err()
        );
    }

    /// With no lower bound there is no span to measure, so nothing is
    /// enforced — the venue decides how far back it goes.
    #[test]
    fn an_open_ended_range_is_not_measured() {
        assert!(SessionRange::until(day(2026, 8, 31)).validate().is_ok());
    }

    #[test]
    fn collections_are_repeated_keys() {
        let query = collections_query(
            &SessionCollection::Equity,
            &[SessionCollection::Cme, SessionCollection::Cfe],
        );

        assert_eq!(
            query.pairs(),
            vec![
                ("instrument-collections[]", "Equity"),
                ("instrument-collections[]", "CME"),
                ("instrument-collections[]", "CFE"),
            ]
        );
    }

    /// Tolerance on the way in, a closed set on the way out.
    ///
    /// A collection the venue adds later still decodes, because `Items<T>`
    /// drops what it cannot parse and a session that vanishes is worse than
    /// one with an unfamiliar name. It cannot be *asked for*: the request
    /// types enumerate what the contract admits, so a typo is a compile error
    /// rather than a 404 after an authenticated round trip.
    #[test]
    fn an_unmodelled_collection_decodes_but_cannot_be_requested() {
        let session: MarketSession = serde_json::from_str(r#"{"instrument-collection": "SMALLS"}"#)
            .expect("a session must not vanish because of its collection");
        assert_eq!(
            session.instrument_collection,
            Some(InstrumentCollection::Unknown("SMALLS".to_string()))
        );

        // And the request side spells only what the contract admits.
        assert_eq!(SessionCollection::Equity.as_wire(), "Equity");
        assert_eq!(FuturesExchange::Cfe.as_wire(), "CFE");
        assert_eq!(
            SessionCollection::from(FuturesExchange::Cme),
            SessionCollection::Cme
        );
    }

    /// Nine calendar months, not two hundred and seventy-nine days.
    ///
    /// `9 * 31` accepted January 1 to October 5, which is longer than nine
    /// months by any reading, and refused ranges the venue would have served.
    #[test]
    fn the_session_range_limit_is_nine_calendar_months() {
        // Exactly nine months is served.
        SessionRange::between(day(2026, 1, 1), day(2026, 10, 1))
            .validate()
            .expect("nine months to the day");

        // One day past it is not, and the old day count accepted it.
        let error = SessionRange::between(day(2026, 1, 1), day(2026, 10, 2))
            .validate()
            .expect_err("nine months and a day");
        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(format!("{error}").contains("2026-10-01"), "{error}");

        // Month lengths do not shift the boundary: adding nine months to the
        // 31st clamps to the last day of the target month rather than
        // overflowing into the next one.
        SessionRange::between(day(2026, 1, 31), day(2026, 10, 31))
            .validate()
            .expect("January 31 to October 31 is nine months");
        assert!(
            SessionRange::between(day(2026, 1, 31), day(2026, 11, 1))
                .validate()
                .is_err()
        );
    }
}
