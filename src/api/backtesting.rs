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
//!
//! One more difference, and it is the one that decides whether any of this
//! works: the backtester answers with **raw** arrays and objects. There is no
//! `data`, no `context` and no `pagination`, so these operations go through the
//! raw verbs rather than the enveloped ones — the shared decoder would have
//! rejected every successful response before its return type was reached.

use crate::TastyTrade;
use crate::api::base::TastyResult;
use crate::api::url::encode_path_segment;
use crate::types::backtest::{
    AvailableDates, BACKTESTER_BASE_URL, Backtest, NewBacktest, SimulateTrade, SimulatedTradePoint,
};

impl TastyTrade {
    /// The identifiers of every backtest this user has run.
    ///
    /// **Identifiers, not runs.** The published contract answers with an array
    /// of strings; fetching each one is [`TastyTrade::backtest`], and doing it
    /// automatically would turn one listing into an unbounded number of
    /// requests without the caller asking.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn backtests(&self) -> TastyResult<Vec<String>> {
        self.get_raw_at(BACKTESTER_BASE_URL, "/backtests").await
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

        self.post_raw_at(BACKTESTER_BASE_URL, "/backtests", backtest)
            .await
    }

    /// One backtest, with whatever progress it has made.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404`.
    pub async fn backtest(&self, id: &str) -> TastyResult<Backtest> {
        self.get_raw_at(
            BACKTESTER_BASE_URL,
            format!("/backtests/{}", encode_path_segment(id)),
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
        self.get_raw_at(
            BACKTESTER_BASE_URL,
            format!("/backtests/{}/logs", encode_path_segment(id)),
        )
        .await
    }

    /// Cancels a running backtest.
    ///
    /// **Mutates server-side state**, though nothing about an account: a
    /// cancelled backtest is a computation stopped, not a position changed.
    ///
    /// Returns nothing, because the venue returns nothing: the published
    /// contract answers `204 No Content`. Asking for a [`Backtest`] back made
    /// this call cancel the computation and then fail parsing an empty body,
    /// so a cancellation that had already happened was reported as an error.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a refusal to cancel a run that
    /// has already finished.
    pub async fn cancel_backtest(&self, id: &str) -> TastyResult<()> {
        self.post_raw_at(
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
        self.get_raw_at(BACKTESTER_BASE_URL, "/available-dates")
            .await
    }

    /// Simulates one trade against historical data.
    ///
    /// Takes existing instruments by symbol and asks what they would have
    /// done. That is a different request from a backtest, which describes
    /// strikes to *select* — a backtest leg has no place here and the venue
    /// does not accept one.
    ///
    /// **Simulates.** Nothing routes and no position changes.
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the trade has no legs, no
    /// underlying, a blank leg symbol, a quantity that is not a whole number
    /// above zero, or a window that ends before it starts.
    pub async fn simulate_trade(
        &self,
        request: &SimulateTrade,
    ) -> TastyResult<Vec<SimulatedTradePoint>> {
        request.validate()?;

        self.post_raw_at(BACKTESTER_BASE_URL, "/simulate-trade", request)
            .await
    }
}
