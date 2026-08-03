//! Server-side strategy backtests.
//!
//! Three things make this area unlike the rest of the crate, and all three are
//! documented rather than smoothed over.
//!
//! It is served by a **different host** — `https://backtester.vast.tastyworks.com`,
//! declared in its own OpenAPI document — where every other area is relative to
//! the configured `base_url`. Its JSON is **camelCase**, not kebab-case. And a
//! backtest is **asynchronous**: create, poll, read logs, cancel. The polling is
//! left to the caller rather than hidden in a blocking helper, because how long
//! to wait and what to do meanwhile is not this library's decision.

use chrono::NaiveDate;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The host the backtesting OpenAPI document declares.
///
/// **One host, not a cert/production pair.** Every other area in this API has
/// two; the backtester's document publishes a single server and no sandbox
/// counterpart. So a backtest run from a certification session reaches the same
/// service as one run from production, and this crate does not invent a second
/// URL to pretend otherwise.
///
/// Errors from it still name the **session's** environment, because that is
/// what a caller needs to know — which credentials were used — and it is
/// derived from the configured `base_url` rather than from the request URL.
pub const BACKTESTER_BASE_URL: &str = "https://backtester.vast.tastyworks.com";

/// What kind of instrument a backtested leg is.
///
/// The document enumerates two values in lowercase. `call` and `put` are not
/// among them — those are [`BacktestSide`], which is a different field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BacktestInstrument {
    /// Shares.
    Equity,
    /// Options on shares.
    EquityOption,
}

/// Which way a backtested leg is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BacktestDirection {
    /// Bought.
    Long,
    /// Sold.
    Short,
}

/// Which side of the option chain a leg is on.
///
/// Only meaningful for [`BacktestInstrument::EquityOption`], which is what the
/// document says and what [`TastyTrade::create_backtest`] checks before it
/// sends anything.
///
/// [`TastyTrade::create_backtest`]: crate::TastyTrade::create_backtest
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BacktestSide {
    /// A call.
    Call,
    /// A put.
    Put,
}

/// How a backtested leg picks its strike.
///
/// Seven values, spelled exactly as the document does — mixed case included,
/// which is why the variants carry explicit renames rather than a container
/// rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrikeSelection {
    /// By target delta.
    #[serde(rename = "delta")]
    Delta,
    /// By percentage out of the money.
    #[serde(rename = "percentageOTM")]
    PercentageOtm,
    /// By percentage out of the money, relative to another leg.
    #[serde(rename = "percentageOTMRelative")]
    PercentageOtmRelative,
    /// By offset from the current price.
    #[serde(rename = "currentPriceOffset")]
    CurrentPriceOffset,
    /// By offset from the current price, relative to another leg.
    #[serde(rename = "currentPriceOffsetRelative")]
    CurrentPriceOffsetRelative,
    /// By exact offset from the current price, relative to another leg.
    #[serde(rename = "currentPriceExactOffsetRelative")]
    CurrentPriceExactOffsetRelative,
    /// By target premium.
    #[serde(rename = "premium")]
    Premium,
}

/// The most contracts or shares a backtested leg may carry.
///
/// The document bounds `quantity` at 1 to 100. Checked locally so a backtest
/// that was always going to be rejected fails before the wait rather than
/// after it.
pub const MAX_BACKTEST_QUANTITY: u32 = 100;

/// One leg of a backtested strategy.
///
/// The venue marks `type`, `direction`, `quantity`, `strikeSelection` and
/// `daysUntilExpiration` required, so those are not `Option`. The strike
/// selectors are: whichever one `strike_selection` names is the one that must
/// be filled in, which the venue validates and this crate does not second-guess
/// — no captured payload shows the mapping.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct BacktestLeg {
    /// Whether the leg is shares or options.
    ///
    /// Not the option side: `call` and `put` belong to
    /// [`BacktestLeg::side`], and putting them here sent a request the venue
    /// rejects.
    #[serde(rename = "type")]
    pub leg_type: BacktestInstrument,
    /// Whether the leg is bought or sold.
    pub direction: BacktestDirection,
    /// How many contracts or shares.
    ///
    /// `Decimal` because every quantity in this crate is, per the money rule —
    /// the backtester types it as an integer bounded 1 to
    /// [`MAX_BACKTEST_QUANTITY`], which is checked locally before the request
    /// is sent, and it is serialized as a JSON number with no fractional part.
    #[serde(with = "crate::types::wire::decimal")]
    pub quantity: Decimal,
    /// How the strike is chosen.
    #[serde(rename = "strikeSelection")]
    pub strike_selection: StrikeSelection,
    /// How many days until the leg expires.
    #[serde(rename = "daysUntilExpiration")]
    pub days_until_expiration: i64,
    /// Which side of the chain, for an option leg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<BacktestSide>,
    /// Which leg the strike is relative to.
    #[serde(
        rename = "strikeRelativeLeg",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub strike_relative_leg: Option<i64>,
    /// Target delta, for delta-selected strikes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::decimal_option"
    )]
    pub delta: Option<Decimal>,
    /// Target distance out of the money, as a percentage.
    #[serde(
        rename = "percentageOTM",
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::decimal_option"
    )]
    pub percentage_otm: Option<Decimal>,
    /// Offset from the current price.
    #[serde(
        rename = "currentPriceOffset",
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::decimal_option"
    )]
    pub current_price_offset: Option<Decimal>,
    /// Target premium.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::decimal_option"
    )]
    pub premium: Option<Decimal>,
}

/// When a backtested strategy opens a trial.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, Default)]
pub struct EntryConditions {
    /// How often to enter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    /// Which days of the week or month, when the frequency needs them.
    #[serde(
        rename = "specificDays",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub specific_days: Vec<i64>,
    /// How many trials may be open at once.
    #[serde(
        rename = "maximumActiveTrials",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub maximum_active_trials: Option<i64>,
    /// What to do when that limit is reached.
    #[serde(
        rename = "maximumActiveTrialsBehavior",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub maximum_active_trials_behavior: Option<String>,
    /// Lowest VIX at which to enter.
    #[serde(
        rename = "minimumVIX",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub minimum_vix: Option<i64>,
    /// Highest VIX at which to enter.
    #[serde(
        rename = "maximumVIX",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub maximum_vix: Option<i64>,
}

/// When a backtested strategy closes a trial.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, Default)]
pub struct ExitConditions {
    /// Close at this much profit, as a percentage.
    #[serde(
        rename = "takeProfitPercentage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub take_profit_percentage: Option<i64>,
    /// Close at this much loss, as a percentage.
    #[serde(
        rename = "stopLossPercentage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub stop_loss_percentage: Option<i64>,
    /// Close after this many days in the trade.
    #[serde(
        rename = "afterDaysInTrade",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub after_days_in_trade: Option<i64>,
    /// Close at this many days to expiration.
    #[serde(
        rename = "atDaysToExpiration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub at_days_to_expiration: Option<i64>,
    /// Close if VIX falls below this.
    #[serde(
        rename = "minimumVIX",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub minimum_vix: Option<i64>,
}

/// A backtest to run.
#[derive(DebugPretty, DisplaySimple, Serialize, Clone)]
pub struct NewBacktest {
    /// The underlying.
    pub symbol: String,
    /// First day of the simulation.
    #[serde(rename = "startDate", with = "crate::types::wire::date")]
    pub start_date: NaiveDate,
    /// Last day of the simulation.
    #[serde(rename = "endDate", with = "crate::types::wire::date")]
    pub end_date: NaiveDate,
    /// The strategy's legs.
    pub legs: Vec<BacktestLeg>,
    /// When to open a trial.
    #[serde(rename = "entryConditions", skip_serializing_if = "Option::is_none")]
    pub entry_conditions: Option<EntryConditions>,
    /// When to close one.
    #[serde(rename = "exitConditions", skip_serializing_if = "Option::is_none")]
    pub exit_conditions: Option<ExitConditions>,
}

impl NewBacktest {
    /// A backtest of `legs` on `symbol` over a date range.
    pub fn new(
        symbol: impl Into<String>,
        start_date: NaiveDate,
        end_date: NaiveDate,
        legs: Vec<BacktestLeg>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            start_date,
            end_date,
            legs,
            entry_conditions: None,
            exit_conditions: None,
        }
    }

    /// Sets when trials open.
    #[must_use]
    pub fn with_entry_conditions(mut self, conditions: EntryConditions) -> Self {
        self.entry_conditions = Some(conditions);
        self
    }

    /// Sets when trials close.
    #[must_use]
    pub fn with_exit_conditions(mut self, conditions: ExitConditions) -> Self {
        self.exit_conditions = Some(conditions);
        self
    }

    /// Fails when the backtest cannot be what the venue accepts.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable. A backtest is long-running, so a request that was always
    /// going to be rejected is worth catching before the wait rather than
    /// after it.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        if self.symbol.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a backtest needs an underlying symbol".to_string(),
            ));
        }
        if self.legs.is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a backtest needs at least one leg; there is no strategy without one".to_string(),
            ));
        }
        if self.start_date > self.end_date {
            return Err(crate::TastyTradeError::Precondition(format!(
                "the backtest starts on {} and ends on {}, which is before it",
                self.start_date, self.end_date
            )));
        }

        for (index, leg) in self.legs.iter().enumerate() {
            // The document bounds quantity at 1 to 100 and types it as an
            // integer. `Decimal` is what every quantity in this crate is, so
            // the range and the whole-number rule are checked here rather than
            // encoded in the field type.
            if leg.quantity <= Decimal::ZERO
                || leg.quantity > Decimal::from(MAX_BACKTEST_QUANTITY)
                || leg.quantity.fract() != Decimal::ZERO
            {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "leg {index} asks for {} contracts; the backtester takes a whole \
                     number from 1 to {MAX_BACKTEST_QUANTITY}",
                    leg.quantity
                )));
            }

            // `side` names call or put, and the document says it applies only
            // to an option leg. Sending it on an equity leg is a request built
            // from a misreading of which field carries which concept.
            match (leg.leg_type, leg.side) {
                (BacktestInstrument::EquityOption, None) => {
                    return Err(crate::TastyTradeError::Precondition(format!(
                        "leg {index} is an option leg with no side; it has to say call \
                         or put"
                    )));
                }
                (BacktestInstrument::Equity, Some(side)) => {
                    return Err(crate::TastyTradeError::Precondition(format!(
                        "leg {index} is an equity leg carrying a {side:?} side; call and \
                         put describe an option"
                    )));
                }
                _ => {}
            }

            if leg.days_until_expiration < 0 {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "leg {index} expires {} days from entry, which is in the past",
                    leg.days_until_expiration
                )));
            }
        }

        Ok(())
    }
}

/// One completed or open trial within a backtest.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct Trial {
    /// When the trial opened, as the venue rendered it.
    ///
    /// `String`: the schema gives it no format, and this service already
    /// disagrees with the rest of the API about JSON casing — assigning a
    /// timestamp type on that evidence would make every trial fail to decode
    /// the first time the guess was wrong.
    #[serde(rename = "openDateTime", default)]
    pub open_date_time: Option<String>,
    /// When it closed.
    #[serde(rename = "closeDateTime", default)]
    pub close_date_time: Option<String>,
    /// What it made or lost.
    #[serde(
        rename = "profitLoss",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub profit_loss: Option<Decimal>,
}

/// One point on a backtest's equity curve.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    /// When, as the venue rendered it.
    #[serde(rename = "dateTime", default)]
    pub date_time: Option<String>,
    /// Cumulative profit or loss.
    #[serde(
        rename = "profitLoss",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub profit_loss: Option<Decimal>,
    /// The underlying's price at that point.
    #[serde(
        rename = "underlyingPrice",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub underlying_price: Option<Decimal>,
}

/// A backtest as the venue reports it.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct Backtest {
    /// The venue's identifier for the run.
    #[serde(default)]
    pub id: Option<String>,
    /// The underlying.
    #[serde(default)]
    pub symbol: Option<String>,
    /// First day of the simulation.
    #[serde(
        rename = "startDate",
        default,
        with = "crate::types::wire::date_option"
    )]
    pub start_date: Option<NaiveDate>,
    /// Last day of the simulation.
    #[serde(rename = "endDate", default, with = "crate::types::wire::date_option")]
    pub end_date: Option<NaiveDate>,
    /// Where the run has got to.
    ///
    /// Left as `String`. `CLAUDE.md`'s rule is that a closed set without a
    /// captured payload produces variants that never match, and no payload for
    /// this service has been captured — the whole area is unreachable from this
    /// checkout. It becomes a `wire_enum!` when a run is observed.
    #[serde(default)]
    pub status: Option<String>,
    /// How far along, as the venue reports it.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub progress: Option<Decimal>,
    /// The venue's estimate of time remaining.
    #[serde(rename = "ETA", default, with = "crate::types::wire::decimal_option")]
    pub eta: Option<Decimal>,
    /// The strategy's legs, echoed back.
    #[serde(default)]
    pub legs: Vec<BacktestLeg>,
    /// When trials opened.
    #[serde(rename = "entryConditions", default)]
    pub entry_conditions: Option<EntryConditions>,
    /// When they closed.
    #[serde(rename = "exitConditions", default)]
    pub exit_conditions: Option<ExitConditions>,
    /// Summary statistics, as the venue publishes no schema for them.
    #[serde(default)]
    pub statistics: Vec<serde_json::Value>,
    /// Each trial the run opened.
    #[serde(default)]
    pub trials: Vec<Trial>,
    /// The equity curve.
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    /// Anything the venue wants a person to read.
    #[serde(default)]
    pub notices: Vec<String>,
}

impl Backtest {
    /// Whether the run has reported a terminal status.
    ///
    /// Deliberately conservative: it answers `true` only for the words this
    /// crate recognises as finished. A status it has not seen is **not**
    /// terminal, because a polling loop that stopped early on an unrecognised
    /// word would abandon a run that was still going.
    pub fn is_finished(&self) -> bool {
        self.status.as_deref().is_some_and(|status| {
            matches!(
                status.trim().to_ascii_lowercase().as_str(),
                "completed" | "complete" | "finished" | "failed" | "cancelled" | "canceled"
            )
        })
    }
}

/// The date range the venue holds data for, per symbol.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct AvailableDates {
    /// The underlying.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Earliest day with data. `String`, since the schema gives it no format.
    #[serde(rename = "startDate", default)]
    pub start_date: Option<String>,
    /// Latest day with data.
    #[serde(rename = "endDate", default)]
    pub end_date: Option<String>,
}

/// One leg of a trade to simulate.
///
/// **Not** a [`BacktestLeg`]. `POST /simulate-trade` takes an existing
/// instrument by symbol and asks what it would have done; a backtest leg
/// describes a strike to *select*. Sending the second where the first belongs
/// is a request the venue does not accept — it carries no `type`,
/// `strikeSelection`, `delta` or `daysUntilExpiration`.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct SimulatedLeg {
    /// The instrument, in the tastytrade OCC symbology.
    pub symbol: String,
    /// Whether it is held long or short.
    pub direction: BacktestDirection,
    /// How many contracts or shares.
    ///
    /// `Decimal` for the same reason as everywhere else; the document types it
    /// as an integer, which is checked locally before the request is sent.
    #[serde(with = "crate::types::wire::decimal")]
    pub quantity: Decimal,
}

/// A trade to run against historical data.
///
/// Modelled from the published request schema rather than passed through as
/// raw JSON: the shape is documented, with three worked examples, so there was
/// nothing to be tolerant about.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct SimulateTrade {
    /// The underlying the trade is on.
    pub underlying: String,
    /// When the trade opens. Optional; the venue picks a default.
    #[serde(
        rename = "startTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::datetime_option"
    )]
    pub start_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// When it closes.
    #[serde(
        rename = "endTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::datetime_option"
    )]
    pub end_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// The legs, each naming an instrument that already exists.
    pub legs: Vec<SimulatedLeg>,
}

impl SimulateTrade {
    /// A simulation of `legs` on `underlying`.
    pub fn new(underlying: impl Into<String>, legs: Vec<SimulatedLeg>) -> Self {
        Self {
            underlying: underlying.into(),
            start_time: None,
            end_time: None,
            legs,
        }
    }

    /// Bounds the simulation in time.
    #[must_use]
    pub fn between(
        mut self,
        start: chrono::DateTime<chrono::FixedOffset>,
        end: chrono::DateTime<chrono::FixedOffset>,
    ) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// Fails when the simulation cannot be what the venue accepts.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        if self.underlying.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a simulated trade needs an underlying symbol".to_string(),
            ));
        }
        if self.legs.is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a simulated trade needs at least one leg".to_string(),
            ));
        }
        for (index, leg) in self.legs.iter().enumerate() {
            if leg.symbol.trim().is_empty() {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "simulated leg {index} has a blank symbol"
                )));
            }
            if leg.quantity <= Decimal::ZERO || leg.quantity.fract() != Decimal::ZERO {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "simulated leg {index} asks for {} contracts; the backtester takes \
                     a whole number above zero",
                    leg.quantity
                )));
            }
        }
        if let (Some(start), Some(end)) = (self.start_time, self.end_time)
            && start > end
        {
            return Err(crate::TastyTradeError::Precondition(
                "the simulation starts after it ends".to_string(),
            ));
        }
        Ok(())
    }
}

/// One point of a simulated trade's history.
///
/// The response is an array of these; the document names four fields and gives
/// no others.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
pub struct SimulatedTradePoint {
    /// When this point is.
    #[serde(rename = "dateTime", default)]
    pub date_time: Option<String>,
    /// What the trade was worth, as a magnitude.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price: Option<Decimal>,
    /// Whether that price is a debit or a credit.
    #[serde(default)]
    pub effect: Option<String>,
    /// The underlying's price at the same moment.
    #[serde(
        rename = "underlyingPrice",
        default,
        with = "crate::types::wire::decimal_option"
    )]
    pub underlying_price: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    fn leg() -> BacktestLeg {
        // A short put: `type` is the instrument and `side` is call or put.
        // Putting "Put" in `type` and "Short" in `direction` — which is what
        // this used to do — sends a request the venue rejects on both fields.
        BacktestLeg {
            leg_type: BacktestInstrument::EquityOption,
            direction: BacktestDirection::Short,
            quantity: Decimal::ONE,
            strike_selection: StrikeSelection::Delta,
            days_until_expiration: 45,
            side: Some(BacktestSide::Put),
            strike_relative_leg: None,
            delta: Some(Decimal::new(16, 2)),
            percentage_otm: None,
            current_price_offset: None,
            premium: None,
        }
    }

    fn backtest() -> NewBacktest {
        NewBacktest::new("SPY", day(2024, 1, 1), day(2024, 12, 31), vec![leg()])
    }

    /// This service spells its JSON in camelCase, unlike every other
    /// tastytrade area.
    #[test]
    fn the_request_serialises_in_the_services_own_casing() {
        let body = serde_json::to_value(backtest()).expect("serialises");

        assert_eq!(body["symbol"], "SPY");
        assert_eq!(body["startDate"], "2024-01-01");
        assert_eq!(body["endDate"], "2024-12-31");
        assert_eq!(body["legs"][0]["strikeSelection"], "delta");
        assert_eq!(body["legs"][0]["daysUntilExpiration"], 45);
        // The document spells these lowercase, and `type` is the instrument
        // rather than the option side.
        assert_eq!(body["legs"][0]["type"], "equity-option");
        assert_eq!(body["legs"][0]["direction"], "short");
        assert_eq!(body["legs"][0]["side"], "put");
        // Unset selectors are omitted rather than sent as null.
        assert!(body["legs"][0].get("premium").is_none(), "{body}");
        assert!(body.get("entryConditions").is_none(), "{body}");
    }

    #[test]
    fn conditions_serialise_in_the_same_casing() {
        let body = serde_json::to_value(
            backtest()
                .with_entry_conditions(EntryConditions {
                    frequency: Some("Daily".to_string()),
                    maximum_active_trials: Some(3),
                    minimum_vix: Some(12),
                    ..EntryConditions::default()
                })
                .with_exit_conditions(ExitConditions {
                    take_profit_percentage: Some(50),
                    at_days_to_expiration: Some(21),
                    ..ExitConditions::default()
                }),
        )
        .expect("serialises");

        assert_eq!(body["entryConditions"]["maximumActiveTrials"], 3);
        assert_eq!(body["entryConditions"]["minimumVIX"], 12);
        assert_eq!(body["exitConditions"]["takeProfitPercentage"], 50);
        assert_eq!(body["exitConditions"]["atDaysToExpiration"], 21);
        // An empty `specificDays` is omitted rather than sent as `[]`.
        assert!(
            body["entryConditions"].get("specificDays").is_none(),
            "{body}"
        );
    }

    /// A backtest is long-running, so a request that was always going to be
    /// rejected is worth catching before the wait rather than after it.
    #[test]
    fn an_impossible_backtest_is_refused_locally() {
        for (what, bad) in [
            (
                "no legs",
                NewBacktest::new("SPY", day(2024, 1, 1), day(2024, 12, 31), vec![]),
            ),
            (
                "an inverted range",
                NewBacktest::new("SPY", day(2024, 12, 31), day(2024, 1, 1), vec![leg()]),
            ),
            (
                "a blank symbol",
                NewBacktest::new("  ", day(2024, 1, 1), day(2024, 12, 31), vec![leg()]),
            ),
        ] {
            let error = bad.validate().expect_err(what);
            assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
            assert!(!error.is_retryable(), "{what}");
        }

        assert!(backtest().validate().is_ok());
    }

    #[test]
    fn a_result_decodes_with_its_trials_and_snapshots() {
        let result: Backtest = serde_json::from_str(
            r#"{"id": "bt-1", "symbol": "SPY", "status": "completed",
                "startDate": "2024-01-01", "endDate": "2024-12-31",
                "progress": 100, "ETA": 0,
                "trials": [{"openDateTime": "2024-01-02T09:30",
                            "closeDateTime": "2024-02-16T16:00",
                            "profitLoss": 125.5}],
                "snapshots": [{"dateTime": "2024-01-02", "profitLoss": 0,
                               "underlyingPrice": 470.1}],
                "notices": ["partial data for one week"]}"#,
        )
        .expect("the backtest must decode");

        assert_eq!(result.id.as_deref(), Some("bt-1"));
        assert_eq!(result.start_date, Some(day(2024, 1, 1)));
        assert_eq!(result.trials.len(), 1);
        assert_eq!(
            result.trials[0].profit_loss.expect("a P&L").to_string(),
            "125.5"
        );
        assert_eq!(
            result.snapshots[0].underlying_price.map(|p| p.to_string()),
            Some("470.1".to_string())
        );
        assert_eq!(result.notices.len(), 1);
        assert!(result.is_finished());
    }

    /// A polling loop that stopped on an unrecognised word would abandon a run
    /// that was still going.
    #[test]
    fn an_unrecognised_status_is_not_treated_as_finished() {
        let running: Backtest =
            serde_json::from_str(r#"{"id": "bt-2", "status": "queued"}"#).expect("decodes");
        assert!(!running.is_finished());

        let unknown: Backtest =
            serde_json::from_str(r#"{"id": "bt-3", "status": "reticulating splines"}"#)
                .expect("decodes");
        assert!(!unknown.is_finished());

        let absent: Backtest = serde_json::from_str(r#"{"id": "bt-4"}"#).expect("decodes");
        assert!(!absent.is_finished());
    }

    /// Only one host is published for this area — there is no sandbox
    /// counterpart, and this crate does not invent one.
    #[test]
    fn the_backtester_host_is_the_one_the_document_declares() {
        assert_eq!(
            BACKTESTER_BASE_URL,
            "https://backtester.vast.tastyworks.com"
        );
    }
}
