//! When the market is open: sessions and holidays.
//!
//! Eleven endpoints in two families. The generic pair takes an instrument
//! collection as a query parameter; the equities and futures families each have
//! their own routes, and the futures ones are keyed by collection in the
//! **path**.

use chrono::NaiveDate;

use crate::TastyTrade;
use crate::api::base::{Items, TastyResult};
use crate::api::query::QueryBuilder;
use crate::api::url::encode_path_segment;
use crate::types::market_time::{
    CurrentMarketSession, InstrumentCollection, MarketCalendar, MarketSession, SessionRange,
    collections_query,
};

impl TastyTrade {
    /// Session timings over a date range.
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the range is inverted or
    /// longer than the nine months the venue answers. Fails when sessions
    /// arrive but none can be decoded; an empty range is `Ok`.
    pub async fn market_sessions(&self, range: &SessionRange) -> TastyResult<Vec<MarketSession>> {
        range.validate()?;

        let query = range.to_query();
        let resp: Items<MarketSession> = self
            .get_with_query("/market-time/sessions", &query.pairs())
            .await?;
        resp.into_items()
    }

    /// The current session for one or more instrument collections.
    ///
    /// `instrument-collections[]` is required by the venue, so `first` is a
    /// separate argument from `rest`: an empty selection is unrepresentable
    /// rather than a runtime `400`.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn current_market_session(
        &self,
        first: InstrumentCollection,
        rest: &[InstrumentCollection],
    ) -> TastyResult<CurrentMarketSession> {
        let query = collections_query(&first, rest);
        self.get_with_query::<CurrentMarketSession, CurrentMarketSession, _>(
            "/market-time/sessions/current",
            &query.pairs(),
        )
        .await
    }

    /// The equities session in progress.
    ///
    /// `current_time` asks the venue what the session was at another moment,
    /// which is the venue's own parameter rather than a local clock trick.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn current_equities_session(
        &self,
        current_time: Option<&str>,
    ) -> TastyResult<CurrentMarketSession> {
        let mut query = QueryBuilder::new();
        query.push_opt("current-time", current_time);

        self.get_with_query::<CurrentMarketSession, CurrentMarketSession, _>(
            "/market-time/equities/sessions/current",
            &query.pairs(),
        )
        .await
    }

    /// The next equities session, optionally after a given day.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn next_equities_session(
        &self,
        date: Option<NaiveDate>,
    ) -> TastyResult<MarketSession> {
        self.session_at("/market-time/equities/sessions/next", date)
            .await
    }

    /// The previous equities session, optionally before a given day.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn previous_equities_session(
        &self,
        date: Option<NaiveDate>,
    ) -> TastyResult<MarketSession> {
        self.session_at("/market-time/equities/sessions/previous", date)
            .await
    }

    /// The equities holiday calendar.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn equities_holidays(&self) -> TastyResult<MarketCalendar> {
        self.get("/market-time/equities/holidays").await
    }

    /// The current session for every futures collection.
    ///
    /// # Errors
    ///
    /// Fails when sessions arrive but none can be decoded.
    pub async fn current_futures_sessions(&self) -> TastyResult<Vec<CurrentMarketSession>> {
        let resp: Items<CurrentMarketSession> =
            self.get("/market-time/futures/sessions/current").await?;
        resp.into_items()
    }

    /// The current session for one futures collection.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn current_futures_session(
        &self,
        collection: &InstrumentCollection,
    ) -> TastyResult<CurrentMarketSession> {
        self.get(format!(
            "/market-time/futures/sessions/current/{}",
            encode_path_segment(collection.as_wire())
        ))
        .await
    }

    /// The next session for one futures collection.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn next_futures_session(
        &self,
        collection: &InstrumentCollection,
        date: Option<NaiveDate>,
    ) -> TastyResult<MarketSession> {
        self.session_at(
            &format!(
                "/market-time/futures/sessions/next/{}",
                encode_path_segment(collection.as_wire())
            ),
            date,
        )
        .await
    }

    /// The previous session for one futures collection.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn previous_futures_session(
        &self,
        collection: &InstrumentCollection,
        date: Option<NaiveDate>,
    ) -> TastyResult<MarketSession> {
        self.session_at(
            &format!(
                "/market-time/futures/sessions/previous/{}",
                encode_path_segment(collection.as_wire())
            ),
            date,
        )
        .await
    }

    /// The holiday calendar for one futures collection.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn futures_holidays(
        &self,
        collection: &InstrumentCollection,
    ) -> TastyResult<MarketCalendar> {
        self.get(format!(
            "/market-time/futures/holidays/{}",
            encode_path_segment(collection.as_wire())
        ))
        .await
    }

    /// The four next/previous lookups differ only in their path.
    ///
    /// Shared so the optional `date` cannot be spelled four different ways —
    /// and so omitting it stays omitting it, which is what leaves the venue's
    /// "relative to now" default in place.
    async fn session_at(&self, path: &str, date: Option<NaiveDate>) -> TastyResult<MarketSession> {
        let mut query = QueryBuilder::new();
        query.push_opt("date", date);

        self.get_with_query::<MarketSession, MarketSession, _>(path, &query.pairs())
            .await
    }
}
