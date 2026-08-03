//! Server-side strategy backtests.
//!
//! The only area served by a **different host**. Everything else about the
//! request is identical to the rest of the crate — same deployment check, same
//! pre-request token refresh, same redacted error, same single place the status
//! is inspected — because the verbs are shared rather than copied.
//!
//! A backtest is asynchronous: create, poll, read logs, cancel. The polling is
//! the caller's, deliberately. How long to wait and what to do meanwhile is not
//! a decision this library gets to make, and a blocking helper would hide a
//! request that can run for minutes.

use crate::TastyTrade;
use crate::api::base::{Items, TastyResult};
use crate::api::url::encode_path_segment;
use crate::types::backtest::{AvailableDates, BACKTESTER_BASE_URL, Backtest, NewBacktest};

impl TastyTrade {
    /// Every backtest this user has run.
    ///
    /// # Errors
    ///
    /// Fails when runs arrive but none can be decoded.
    pub async fn backtests(&self) -> TastyResult<Vec<Backtest>> {
        let resp: Items<Backtest> = self
            .get_with_query_at(BACKTESTER_BASE_URL, "/backtests", &[])
            .await?;
        resp.into_items()
    }

    /// Starts a backtest.
    ///
    /// Returns as soon as the venue accepts the run; the result arrives by
    /// polling [`TastyTrade::backtest`] until
    /// [`crate::prelude::Backtest::is_finished`].
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the backtest has no legs,
    /// no symbol, or an inverted date range. A backtest is long-running, so a
    /// request that was always going to be rejected is worth catching before
    /// the wait rather than after it.
    pub async fn create_backtest(&self, backtest: &NewBacktest) -> TastyResult<Backtest> {
        backtest.validate()?;

        self.post_at(BACKTESTER_BASE_URL, "/backtests", backtest)
            .await
    }

    /// One backtest, with whatever progress it has made.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404`.
    pub async fn backtest(&self, id: &str) -> TastyResult<Backtest> {
        self.get_with_query_at(
            BACKTESTER_BASE_URL,
            format!("/backtests/{}", encode_path_segment(id)),
            &[],
        )
        .await
    }

    /// A backtest's logs.
    ///
    /// Returned as the venue's own JSON: no schema is published for them, and a
    /// type invented for a log is a type that stops decoding the first time the
    /// venue adds a field.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn backtest_logs(&self, id: &str) -> TastyResult<serde_json::Value> {
        self.get_with_query_at(
            BACKTESTER_BASE_URL,
            format!("/backtests/{}/logs", encode_path_segment(id)),
            &[],
        )
        .await
    }

    /// Cancels a running backtest.
    ///
    /// **Mutates server-side state**, though nothing about an account: a
    /// cancelled backtest is a computation stopped, not a position changed.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a refusal to cancel a run that
    /// has already finished.
    pub async fn cancel_backtest(&self, id: &str) -> TastyResult<Backtest> {
        self.post_at(
            BACKTESTER_BASE_URL,
            format!("/backtests/{}/cancel", encode_path_segment(id)),
            serde_json::Value::Object(serde_json::Map::new()),
        )
        .await
    }

    /// Which date ranges the venue holds data for.
    ///
    /// # Errors
    ///
    /// Fails when ranges arrive but none can be decoded.
    pub async fn available_dates(&self) -> TastyResult<Vec<AvailableDates>> {
        let resp: Items<AvailableDates> = self
            .get_with_query_at(BACKTESTER_BASE_URL, "/available-dates", &[])
            .await?;
        resp.into_items()
    }

    /// Simulates one trade.
    ///
    /// The body is passed through as JSON: the published document describes
    /// this endpoint's request only as an object, so there is nothing to model
    /// against and a guessed type would refuse requests the venue accepts.
    /// It becomes a modelled type once a real payload is captured.
    ///
    /// **Simulates.** Nothing routes and no position changes.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn simulate_trade(
        &self,
        request: &serde_json::Value,
    ) -> TastyResult<serde_json::Value> {
        self.post_at(BACKTESTER_BASE_URL, "/simulate-trade", request)
            .await
    }
}
