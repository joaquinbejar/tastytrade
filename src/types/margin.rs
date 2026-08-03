//! What an order will consume, and what the account is allowed to hold.
//!
//! Two areas that answer the question a trading client asks before every order:
//! how much buying power will this take, and am I permitted to hold it. The
//! only way to find out before this existed was to dry-run the order, which
//! tells you about one specific order and nothing about the account's standing
//! requirements or position limits.
//!
//! Every figure is money or a ratio, so every figure is `Decimal`. The
//! requirements report is nested three levels deep — total, then per
//! underlying, then per margin strategy — and it is modelled that way rather
//! than flattened, because the per-strategy numbers are what explain the total.

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::accounts::AccountNumber;
use crate::types::instrument::InstrumentType;
use crate::types::order::{Action, OrderType, PriceEffect, TimeInForce};

/// The account's current margin and capital requirements, grouped by
/// underlying.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginRequirementsReport {
    /// Which account this is about. Account PII.
    #[serde(default)]
    pub account_number: Option<AccountNumber>,
    /// What this row covers, e.g. `Total`.
    #[serde(default)]
    pub description: Option<String>,
    /// How margin is calculated, e.g. `Reg T`.
    #[serde(default)]
    pub margin_calculation_type: Option<String>,
    /// What the account may do with options.
    #[serde(default)]
    pub option_level: Option<String>,
    /// Total margin required.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_requirement: Option<Decimal>,
    /// Whether that requirement is a debit or a credit.
    #[serde(default)]
    pub margin_requirement_effect: Option<PriceEffect>,
    /// Total maintenance required.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_requirement: Option<Decimal>,
    /// Whether the maintenance requirement is a debit or a credit.
    #[serde(default)]
    pub maintenance_requirement_effect: Option<PriceEffect>,
    /// Margin equity.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_equity: Option<Decimal>,
    /// Whether the margin equity is a debit or a credit.
    #[serde(default)]
    pub margin_equity_effect: Option<PriceEffect>,
    /// Buying power available for options.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub option_buying_power: Option<Decimal>,
    /// Whether the option buying power is a debit or a credit.
    #[serde(default)]
    pub option_buying_power_effect: Option<PriceEffect>,
    /// The Reg T margin requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub reg_t_margin_requirement: Option<Decimal>,
    /// Whether the Reg T requirement is a debit or a credit.
    #[serde(default)]
    pub reg_t_margin_requirement_effect: Option<PriceEffect>,
    /// Reg T option buying power.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub reg_t_option_buying_power: Option<Decimal>,
    /// Whether the Reg T option buying power is a debit or a credit.
    #[serde(default)]
    pub reg_t_option_buying_power_effect: Option<PriceEffect>,
    /// Equity above the maintenance requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_excess: Option<Decimal>,
    /// Whether the maintenance excess is a debit or a credit.
    #[serde(default)]
    pub maintenance_excess_effect: Option<PriceEffect>,
    /// One entry per underlying the account holds.
    #[serde(default)]
    pub groups: Vec<MarginGroup>,
    /// When the venue last recalculated, as epoch milliseconds.
    ///
    /// Left as an integer: the venue sends a number and does not document its
    /// epoch or unit, and a `DateTime` built on a guess is worse than a number
    /// a caller can convert once they know.
    #[serde(default)]
    pub last_state_timestamp: Option<i64>,
}

/// The requirements attributable to one underlying.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginGroup {
    /// Human-readable name.
    #[serde(default)]
    pub description: Option<String>,
    /// The product code.
    #[serde(default)]
    pub code: Option<String>,
    /// The underlying.
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// What kind of instrument the underlying is.
    #[serde(default)]
    pub underlying_type: Option<String>,
    /// How margin is calculated for it.
    #[serde(default)]
    pub margin_calculation_type: Option<String>,
    /// Margin required for this underlying.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_requirement: Option<Decimal>,
    /// Whether that requirement is a debit or a credit.
    #[serde(default)]
    pub margin_requirement_effect: Option<PriceEffect>,
    /// Maintenance required for this underlying.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_requirement: Option<Decimal>,
    /// Whether the maintenance requirement is a debit or a credit.
    #[serde(default)]
    pub maintenance_requirement_effect: Option<PriceEffect>,
    /// Buying power consumed.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub buying_power: Option<Decimal>,
    /// Whether the buying power figure is a debit or a credit.
    #[serde(default)]
    pub buying_power_effect: Option<PriceEffect>,
    /// How far the venue stresses the price upward, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price_increase_percent: Option<Decimal>,
    /// How far the venue stresses the price downward, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price_decrease_percent: Option<Decimal>,
    /// One entry per margin strategy within this underlying.
    ///
    /// This is where the total comes from, which is why the nesting is kept:
    /// flattening it would leave a number with no explanation.
    #[serde(default)]
    pub groups: Vec<MarginStrategyGroup>,
}

/// The requirements attributable to one margin strategy within an underlying.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginStrategyGroup {
    /// The strategy, e.g. `LONG_UNDERLYING`.
    #[serde(default)]
    pub description: Option<String>,
    /// Margin required for it.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_requirement: Option<Decimal>,
    /// Whether that requirement is a debit or a credit.
    #[serde(default)]
    pub margin_requirement_effect: Option<PriceEffect>,
    /// Maintenance required for it.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_requirement: Option<Decimal>,
    /// Whether the maintenance requirement is a debit or a credit.
    #[serde(default)]
    pub maintenance_requirement_effect: Option<PriceEffect>,
    /// Buying power consumed.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub buying_power: Option<Decimal>,
    /// Whether the buying power figure is a debit or a credit.
    #[serde(default)]
    pub buying_power_effect: Option<PriceEffect>,
    /// Whether a working order is counted in this figure.
    #[serde(default)]
    pub includes_working_order: Option<bool>,
    /// The positions the strategy is made of.
    #[serde(default)]
    pub position_entries: Vec<MarginPositionEntry>,
}

/// One position inside a margin strategy.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginPositionEntry {
    /// The instrument.
    #[serde(default)]
    pub instrument_symbol: Option<String>,
    /// What kind of instrument it is.
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
    /// How many units.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub quantity: Option<Decimal>,
    /// The closing price used.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub close_price: Option<Decimal>,
    /// The fixing price, when the instrument has one.
    ///
    /// The venue sends the literal string `NaN` for an instrument that does not
    /// fix, which is not a number `Decimal` can hold — that is the point of a
    /// fixed-point type. It decodes as `None`, and anything else unparseable is
    /// still an error.
    #[serde(default, with = "crate::types::wire::decimal_option_nan")]
    pub fixing_price: Option<Decimal>,
}

/// What one order would do to the account's margin.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginEstimate {
    /// The account's margin as things stand.
    #[serde(default)]
    pub base_results: Option<MarginImpact>,
    /// The account's margin with the proposed order included.
    #[serde(default)]
    pub new_order_results: Option<MarginImpact>,
    /// The venue's most recent calculation, which may predate the request.
    #[serde(default)]
    pub last_results: Option<MarginImpact>,
    /// How much more margin the order needs.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub change_in_margin_requirement: Option<Decimal>,
    /// Whether that change is a debit or a credit.
    #[serde(default)]
    pub change_in_margin_requirement_effect: Option<PriceEffect>,
    /// How much buying power the order consumes.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub change_in_buying_power: Option<Decimal>,
    /// Whether that change is a debit or a credit.
    #[serde(default)]
    pub change_in_buying_power_effect: Option<PriceEffect>,
    /// Buying power before the order.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub current_buying_power: Option<Decimal>,
    /// Whether that figure is a debit or a credit.
    #[serde(default)]
    pub current_buying_power_effect: Option<PriceEffect>,
    /// Buying power after the order.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub new_buying_power: Option<Decimal>,
    /// Whether that figure is a debit or a credit.
    #[serde(default)]
    pub new_buying_power_effect: Option<PriceEffect>,
    /// What the order would require on its own, ignoring the rest of the book.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub isolated_order_margin_requirement: Option<Decimal>,
    /// Whether that figure is a debit or a credit.
    #[serde(default)]
    pub isolated_order_margin_requirement_effect: Option<PriceEffect>,
    /// Whether the venue recognises the order as a spread.
    #[serde(default)]
    pub is_spread: Option<bool>,
    /// The orders the estimate covered, as the venue echoed them.
    #[serde(default)]
    pub orders: Vec<MarginDryRunOrder>,
    /// The identifiers the estimate covered.
    #[serde(default)]
    pub order_ids: Vec<String>,
}

/// One snapshot of an account's margin.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginImpact {
    /// The product code.
    #[serde(default)]
    pub code: Option<String>,
    /// Human-readable name.
    #[serde(default)]
    pub description: Option<String>,
    /// How many entities the calculation covered.
    #[serde(default)]
    pub entity_count: Option<i64>,
    /// The underlying.
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// What kind of instrument the underlying is.
    #[serde(default)]
    pub underlying_type: Option<String>,
    /// The price used for the underlying.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub underlying_price: Option<Decimal>,
    /// Margin required.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_requirement: Option<Decimal>,
    /// Whether that requirement is a debit or a credit.
    #[serde(default)]
    pub margin_requirement_effect: Option<PriceEffect>,
    /// Maintenance required.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_requirement: Option<Decimal>,
    /// Whether the maintenance requirement is a debit or a credit.
    #[serde(default)]
    pub maintenance_requirement_effect: Option<PriceEffect>,
    /// The effect on buying power.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub buying_power_impact: Option<Decimal>,
    /// Whether that impact is a debit or a credit.
    #[serde(default)]
    pub buying_power_impact_effect: Option<PriceEffect>,
    /// The requirement before adjustments.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub base_margin_requirement: Option<Decimal>,
    /// Whether the base requirement is a debit or a credit.
    #[serde(default)]
    pub base_margin_requirement_effect: Option<PriceEffect>,
    /// How margin is calculated.
    #[serde(default)]
    pub margin_calculation_type: Option<String>,
    /// Cash adjustment applied.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cash_adjustment: Option<Decimal>,
    /// Whether the cash adjustment is a debit or a credit.
    #[serde(default)]
    pub cash_adjustment_effect: Option<PriceEffect>,
    /// Margin held against working orders.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub working_margin: Option<Decimal>,
    /// Whether the working margin is a debit or a credit.
    #[serde(default)]
    pub working_margin_effect: Option<PriceEffect>,
    /// Margin held against open positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub position_margin: Option<Decimal>,
    /// Whether the position margin is a debit or a credit.
    #[serde(default)]
    pub position_margin_effect: Option<PriceEffect>,
    /// Equity adjustment attributable to positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub position_equity_adjustment: Option<Decimal>,
    /// Whether that adjustment is a debit or a credit.
    #[serde(default)]
    pub position_equity_adjustment_effect: Option<PriceEffect>,
    /// Value settled in cash.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cash_settlement_value: Option<Decimal>,
    /// Whether the cash settlement value is a debit or a credit.
    #[serde(default)]
    pub cash_settlement_value_effect: Option<PriceEffect>,
    /// Value of long equity positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_equity_value: Option<Decimal>,
    /// Value of short equity positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_equity_value: Option<Decimal>,
    /// Value of long derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_derivative_value: Option<Decimal>,
    /// Value of short derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_derivative_value: Option<Decimal>,
    /// Value of long cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_cryptocurrency_value: Option<Decimal>,
    /// Value of short cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_cryptocurrency_value: Option<Decimal>,
    /// The per-strategy breakdown.
    #[serde(default)]
    pub group_results: Vec<MarginGroupResult>,
    /// The orders the snapshot covered.
    #[serde(default)]
    pub order_ids: Vec<String>,
    /// When the venue calculated it, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub calculated_at: Option<DateTime<FixedOffset>>,
    /// The marks used, by symbol.
    #[serde(default, with = "crate::types::wire::decimal_map")]
    pub marks: Option<HashMap<String, Decimal>>,
}

/// One strategy's contribution to a [`MarginImpact`].
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginGroupResult {
    /// Which strategy, and what it is made of.
    #[serde(default)]
    pub group: Option<MarginStrategy>,
    /// Margin required for it.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin: Option<Decimal>,
    /// Maintenance required for it.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_requirement: Option<Decimal>,
    /// Whether the maintenance requirement is a debit or a credit.
    #[serde(default)]
    pub maintenance_requirement_effect: Option<PriceEffect>,
    /// The net liquidating value the strategy contributes.
    #[serde(default)]
    pub net_liq_result: Option<NetLiquidatingValues>,
}

/// A margin strategy and the position it rests on.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginStrategy {
    /// The strategy, e.g. `LONG_UNDERLYING`.
    #[serde(default)]
    pub margin_strategy: Option<String>,
    /// The underlying position.
    #[serde(default)]
    pub underlying_entry: Option<MarginPositionEntry>,
}

/// The long and short values a strategy contributes.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct NetLiquidatingValues {
    /// Value of long equity positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_equity_value: Option<Decimal>,
    /// Value of short equity positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_equity_value: Option<Decimal>,
    /// Value of long derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_derivative_value: Option<Decimal>,
    /// Value of short derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_derivative_value: Option<Decimal>,
    /// Value of long cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_cryptocurrency_value: Option<Decimal>,
    /// Value of short cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_cryptocurrency_value: Option<Decimal>,
}

/// An order as the margin dry run echoes it back.
///
/// Not a [`crate::prelude::LiveOrderRecord`]: its `id` is a string like
/// `dry-run-0` rather than the venue-assigned integer, because nothing was
/// placed.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginDryRunOrder {
    /// The venue's placeholder identifier, e.g. `dry-run-0`.
    #[serde(default)]
    pub id: Option<String>,
    /// Which account it would be placed against. Account PII.
    #[serde(default)]
    pub account_number: Option<AccountNumber>,
    /// How long it would rest.
    #[serde(default)]
    pub time_in_force: Option<String>,
    /// What kind of order it is.
    #[serde(default)]
    pub order_type: Option<String>,
    /// The underlying.
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// The limit or stop price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price: Option<Decimal>,
    /// Whether the price is a debit or a credit.
    #[serde(default)]
    pub price_effect: Option<PriceEffect>,
    /// The order's notional value.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub value: Option<Decimal>,
    /// Whether the value is a debit or a credit.
    #[serde(default)]
    pub value_effect: Option<PriceEffect>,
    /// The order's size.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub size: Option<Decimal>,
    /// Whether it could be cancelled if it were real.
    #[serde(default)]
    pub cancellable: Option<bool>,
    /// Whether it could be edited if it were real.
    #[serde(default)]
    pub editable: Option<bool>,
    /// Its legs.
    #[serde(default)]
    pub legs: Vec<MarginDryRunLeg>,
}

/// One leg of a [`MarginDryRunOrder`].
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginDryRunLeg {
    /// The instrument.
    #[serde(default)]
    pub symbol: Option<String>,
    /// What kind of instrument it is.
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
    /// How many units.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub quantity: Option<Decimal>,
    /// How many are still unfilled.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub remaining_quantity: Option<Decimal>,
    /// What the leg does to a position.
    #[serde(default)]
    pub action: Option<String>,
}

/// The standing margin requirement for one underlying.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct EffectiveMarginRequirement {
    /// The underlying.
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// The clearing firm's identifier.
    #[serde(default)]
    pub clearing_identifier: Option<String>,
    /// Whether the requirement has been withdrawn.
    #[serde(default)]
    pub is_deleted: Option<bool>,
    /// Initial requirement on a long equity position, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_equity_initial: Option<Decimal>,
    /// Maintenance requirement on a long equity position, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_equity_maintenance: Option<Decimal>,
    /// Initial requirement on a short equity position, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_equity_initial: Option<Decimal>,
    /// Maintenance requirement on a short equity position, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_equity_maintenance: Option<Decimal>,
    /// Floor applied to a naked option requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub naked_option_floor: Option<Decimal>,
    /// Minimum naked option requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub naked_option_minimum: Option<Decimal>,
    /// Standard naked option requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub naked_option_standard: Option<Decimal>,
}

/// How much of each instrument type an account may order and hold.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PositionLimit {
    /// The venue's identifier for this limit record.
    #[serde(default)]
    pub id: Option<i64>,
    /// Which account this is about. Account PII.
    #[serde(default)]
    pub account_number: Option<AccountNumber>,
    /// Largest single equity order.
    #[serde(default)]
    pub equity_order_size: Option<i64>,
    /// Largest equity position.
    #[serde(default)]
    pub equity_position_size: Option<i64>,
    /// Largest single equity-option order.
    #[serde(default)]
    pub equity_option_order_size: Option<i64>,
    /// Largest equity-option position.
    #[serde(default)]
    pub equity_option_position_size: Option<i64>,
    /// Largest single futures order.
    #[serde(default)]
    pub future_order_size: Option<i64>,
    /// Largest futures position.
    #[serde(default)]
    pub future_position_size: Option<i64>,
    /// Largest single future-option order.
    #[serde(default)]
    pub future_option_order_size: Option<i64>,
    /// Largest future-option position.
    #[serde(default)]
    pub future_option_position_size: Option<i64>,
    /// Largest notional value of a single event-contract order.
    #[serde(default)]
    pub event_contract_notional_order_limit: Option<i64>,
    /// Largest total notional value of event-contract positions.
    #[serde(default)]
    pub event_contract_total_notional_position_value: Option<i64>,
    /// How many opening orders may be outstanding on one underlying.
    #[serde(default)]
    pub underlying_opening_order_limit: Option<i64>,
}

/// The venue's public margin configuration.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginConfiguration {
    /// The risk-free rate the venue prices against, as a ratio.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub risk_free_rate: Option<Decimal>,
}

/// One row of a SPAN risk file.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct SpanRow {
    /// Which exchange the file came from.
    #[serde(default)]
    pub exchange: Option<String>,
    /// Which day the file is for.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub file_date: Option<NaiveDate>,
    /// The row's position in the file.
    #[serde(default)]
    pub row_index: Option<i64>,
    /// The row itself, as the exchange wrote it.
    ///
    /// Left as text on purpose: a SPAN row is a fixed-width record in the
    /// exchange's own format, and parsing it is a different job from talking to
    /// this API.
    #[serde(default)]
    pub row_data: Option<String>,
}

/// Which exchange's SPAN risk rows to ask for.
///
/// The published contract closes this parameter to two values. Accepting any
/// `&str` made a typo and a blank string representable on a parameter the
/// venue requires, so the mistake surfaced as a `400` after an authenticated
/// round trip instead of at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanExchange {
    /// CME Group.
    Cme,
    /// Cboe Futures Exchange.
    Cfe,
}

impl SpanExchange {
    /// The spelling the venue expects.
    pub fn as_wire(&self) -> &'static str {
        match self {
            SpanExchange::Cme => "CME",
            SpanExchange::Cfe => "CFE",
        }
    }
}

impl std::fmt::Display for SpanExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// An order to estimate margin for.
///
/// Deliberately **not** [`crate::prelude::Order`]. The margin endpoint requires
/// an account number and an underlying symbol that the order type does not
/// carry, and serialising an `Order` into this body would send a request
/// missing two required fields. It also cannot route: there is no path from
/// here to a placement.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginOrderRequest {
    /// Which account to estimate against.
    pub account_number: AccountNumber,
    /// The underlying the order is on.
    pub underlying_symbol: String,
    /// What kind of instrument the underlying is.
    pub underlying_instrument_type: InstrumentType,
    /// How long the order would rest.
    pub time_in_force: TimeInForce,
    /// What kind of order it is.
    pub order_type: OrderType,
    /// The limit or stop price, when the order type takes one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price: Option<Decimal>,
    /// Whether the price is a debit or a credit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub price_effect: Option<PriceEffect>,
    /// When a `GTD` order would expire.
    ///
    /// The venue requires it for that time in force and ignores it otherwise.
    /// Estimating a dated order without the date estimates a different order.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "crate::types::wire::date_option")]
    pub gtc_date: Option<NaiveDate>,
    /// The trigger price, for the stop order types.
    ///
    /// Separate from `price`, which is the working price a `StopLimit` fills
    /// at. An estimate that dropped this described an order with no trigger.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub stop_trigger: Option<Decimal>,
    /// The order this one would replace, when estimating a replacement.
    ///
    /// A replacement's margin is not a new order's margin: the venue nets it
    /// against what the existing order already reserves.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub replaces_order_id: Option<String>,
    /// The legs, one to four of them.
    pub legs: Vec<MarginOrderLeg>,
}

/// One leg of a [`MarginOrderRequest`].
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct MarginOrderLeg {
    /// The instrument.
    pub symbol: String,
    /// What kind of instrument it is.
    pub instrument_type: InstrumentType,
    /// How many units. Fractional, because cryptocurrencies are.
    #[serde(with = "crate::types::wire::decimal")]
    pub quantity: Decimal,
    /// What the leg does to a position.
    pub action: Action,
    /// How much of the leg is still working, when estimating a replacement.
    ///
    /// A partially filled leg reserves margin for what is left, not for what
    /// was originally asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub remaining_quantity: Option<Decimal>,
}

/// The most legs the venue accepts on a margin estimate.
pub const MAX_MARGIN_LEGS: usize = 4;

impl MarginOrderRequest {
    /// A request for `account_number` on `underlying_symbol`.
    pub fn new(
        account_number: impl Into<AccountNumber>,
        underlying_symbol: impl Into<String>,
        underlying_instrument_type: InstrumentType,
        order_type: OrderType,
        time_in_force: TimeInForce,
        legs: Vec<MarginOrderLeg>,
    ) -> Self {
        Self {
            account_number: account_number.into(),
            underlying_symbol: underlying_symbol.into(),
            underlying_instrument_type,
            time_in_force,
            order_type,
            price: None,
            price_effect: None,
            gtc_date: None,
            stop_trigger: None,
            replaces_order_id: None,
            legs,
        }
    }

    /// Sets the price and its direction.
    #[must_use]
    pub fn with_price(mut self, price: Decimal, effect: PriceEffect) -> Self {
        self.price = Some(price);
        self.price_effect = Some(effect);
        self
    }

    /// Sets the day a `GTD` order would expire.
    #[must_use]
    pub fn with_gtc_date(mut self, gtc_date: NaiveDate) -> Self {
        self.gtc_date = Some(gtc_date);
        self
    }

    /// Sets the trigger price for a stop order type.
    #[must_use]
    pub fn with_stop_trigger(mut self, stop_trigger: Decimal) -> Self {
        self.stop_trigger = Some(stop_trigger);
        self
    }

    /// Estimates a replacement of an existing order rather than a new one.
    #[must_use]
    pub fn replacing(mut self, order_id: impl Into<String>) -> Self {
        self.replaces_order_id = Some(order_id.into());
        self
    }

    /// Fails when the request cannot be what the venue accepts.
    ///
    /// Local checks only, all of them
    /// [`crate::TastyTradeError::Precondition`] and therefore not retryable:
    /// nothing was sent, and sending it again would fail the same way.
    pub(crate) fn validate(&self, account_number: &str) -> crate::TastyResult<()> {
        if self.account_number.0 != account_number {
            // The body carries the account, and so does the path. If they
            // disagree the venue has to pick one, and which one it picks is not
            // something to find out on a margin figure somebody sizes a
            // position from.
            return Err(crate::TastyTradeError::Precondition(
                "the margin request names a different account from the one it is being \
                 sent to; build the request from this account"
                    .to_string(),
            ));
        }

        if self.underlying_symbol.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a margin estimate needs an underlying symbol, and this one is blank".to_string(),
            ));
        }

        if self.legs.is_empty() || self.legs.len() > MAX_MARGIN_LEGS {
            return Err(crate::TastyTradeError::Precondition(format!(
                "a margin estimate takes one to {MAX_MARGIN_LEGS} legs, and this one has {}",
                self.legs.len()
            )));
        }

        // Duplicates are rejected because the venue documents the legs as
        // unique. Two identical legs is almost always one leg written twice,
        // and a doubled requirement is the kind of wrong that looks plausible.
        for (index, leg) in self.legs.iter().enumerate() {
            if leg.symbol.trim().is_empty() {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "leg {index} has a blank symbol"
                )));
            }
            if self.legs[..index].contains(leg) {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "leg {index} duplicates an earlier one; the venue takes unique legs, and \
                     a repeated leg doubles the requirement it reports"
                )));
            }
            if let Some(remaining) = leg.remaining_quantity
                && (remaining <= Decimal::ZERO || remaining > leg.quantity)
            {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "leg {index} says {remaining} of {} is still working, which is not a \
                     part of it; a replacement reserves margin for what is left",
                    leg.quantity
                )));
            }
        }

        self.validate_price()?;

        Ok(())
    }

    /// The price and trigger rules, matching what an order is held to.
    ///
    /// An estimate of an order the venue would refuse is not an estimate of
    /// anything, and getting it wrong here produces a plausible buying-power
    /// figure for an order that never existed. [`crate::prelude::Order`] checks
    /// the same rules before placement; this is the same table.
    fn validate_price(&self) -> crate::TastyResult<()> {
        // Exhaustive with no wildcard arm, so a variant added later breaks
        // this build rather than inheriting "any price is fine".
        let (needs_price, needs_trigger) = match self.order_type {
            OrderType::Limit | OrderType::MarketableLimit => (true, false),
            OrderType::StopLimit => (true, true),
            OrderType::Stop => (false, true),
            OrderType::NotionalMarket => (true, false),
            OrderType::Market => (false, false),
        };

        if let Some(price) = self.price {
            if needs_price && price <= Decimal::ZERO {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "a {:?} order needs a price greater than zero, got {price}",
                    self.order_type
                )));
            }
            if !needs_price && price != Decimal::ZERO {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "a {:?} order carries no price, so price must be zero, got {price}",
                    self.order_type
                )));
            }
        } else if needs_price {
            return Err(crate::TastyTradeError::Precondition(format!(
                "a {:?} order needs a price, and this estimate has none",
                self.order_type
            )));
        }

        match (needs_trigger, self.stop_trigger) {
            (true, None) => {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "a {:?} order is defined by its trigger, and this estimate has none",
                    self.order_type
                )));
            }
            (true, Some(trigger)) if trigger <= Decimal::ZERO => {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "a stop trigger of {trigger} either never fires or fires immediately"
                )));
            }
            // A trigger on an order type that has none is a misunderstanding
            // worth naming rather than a field the venue quietly ignores.
            (false, Some(trigger)) => {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "a {:?} order has no trigger, and this estimate sets one ({trigger})",
                    self.order_type
                )));
            }
            _ => {}
        }

        if matches!(self.time_in_force, TimeInForce::Gtd) && self.gtc_date.is_none() {
            return Err(crate::TastyTradeError::Precondition(
                "a GTD order expires on a day, and this estimate does not say which".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    const REQUIREMENTS: &str = include_str!("../../Doc/margin_requirements.json");
    const DRY_RUN: &str = include_str!("../../Doc/margin_dry_run.json");

    fn leg(symbol: &str) -> MarginOrderLeg {
        MarginOrderLeg {
            symbol: symbol.to_string(),
            instrument_type: InstrumentType::Equity,
            quantity: Decimal::from(1),
            action: Action::BuyToOpen,
            remaining_quantity: None,
        }
    }

    fn request(legs: Vec<MarginOrderLeg>) -> MarginOrderRequest {
        // A `Limit` needs a working price, the same rule an order is held to.
        MarginOrderRequest::new(
            "SENTINEL-5WX00042",
            "AAPL",
            InstrumentType::Equity,
            OrderType::Limit,
            TimeInForce::Day,
            legs,
        )
        .with_price(Decimal::from(100), PriceEffect::Debit)
    }

    #[test]
    fn the_requirements_report_decodes_with_its_nesting_intact() {
        let body: serde_json::Value = serde_json::from_str(REQUIREMENTS).expect("valid JSON");
        let report: MarginRequirementsReport =
            serde_json::from_value(body["data"].clone()).expect("the report must decode");

        assert_eq!(report.description.as_deref(), Some("Total"));
        assert_eq!(
            report
                .margin_requirement
                .expect("a requirement")
                .to_string(),
            "4337.831401355",
            "every digit the venue sent must survive"
        );

        // Three levels: total, per underlying, per strategy. Flattening this
        // would leave the total with no explanation.
        assert_eq!(report.groups.len(), 2);
        let apple = &report.groups[0];
        assert_eq!(apple.underlying_symbol.as_deref(), Some("AAPL"));
        assert!(!apple.groups.is_empty());
        let strategy = &apple.groups[0];
        assert_eq!(strategy.description.as_deref(), Some("LONG_UNDERLYING"));
        assert_eq!(strategy.position_entries.len(), 1);
        assert_eq!(
            strategy.position_entries[0].instrument_symbol.as_deref(),
            Some("AAPL")
        );
    }

    /// The venue sends the literal string `NaN` for a fixing price that does
    /// not apply. `Decimal` is fixed-point and has no such value, so it decodes
    /// as `None` — and without that the whole report fails to decode.
    #[test]
    fn a_nan_fixing_price_decodes_as_absent_rather_than_failing() {
        let body: serde_json::Value = serde_json::from_str(REQUIREMENTS).expect("valid JSON");
        let report: MarginRequirementsReport =
            serde_json::from_value(body["data"].clone()).expect("the report must decode");

        let entry = &report.groups[0].groups[0].position_entries[0];
        assert_eq!(entry.fixing_price, None);
        // …and a real one still decodes.
        assert_eq!(entry.close_price.expect("a close price").to_string(), "0.0");
    }

    /// Anything else unparseable is still an error: a helper that swallowed
    /// every bad value would turn a decoding bug into a silently absent price
    /// on a margin report.
    #[test]
    fn only_nan_is_forgiven() {
        let ok: MarginPositionEntry =
            serde_json::from_str(r#"{"fixing-price": "NaN"}"#).expect("NaN is absent");
        assert_eq!(ok.fixing_price, None);

        assert!(
            serde_json::from_str::<MarginPositionEntry>(r#"{"fixing-price": "banana"}"#).is_err(),
            "garbage must not silently become an absent price"
        );
    }

    #[test]
    fn the_dry_run_result_decodes_including_the_marks() {
        let body: serde_json::Value = serde_json::from_str(DRY_RUN).expect("valid JSON");
        let estimate: MarginEstimate =
            serde_json::from_value(body["data"].clone()).expect("the estimate must decode");

        let new = estimate
            .new_order_results
            .as_ref()
            .expect("a new-order result");
        assert_eq!(
            new.buying_power_impact.expect("an impact").to_string(),
            "22094.668598645"
        );
        let marks = new.marks.as_ref().expect("marks");
        assert_eq!(marks.get("AAPL").expect("a mark").to_string(), "196.03");

        assert_eq!(estimate.is_spread, Some(false));
        assert_eq!(estimate.orders.len(), 1);
        assert_eq!(estimate.orders[0].id.as_deref(), Some("dry-run-0"));
        assert_eq!(estimate.orders[0].legs.len(), 1);
    }

    #[test]
    fn a_request_for_another_account_is_refused_locally() {
        let error = request(vec![leg("AAPL")])
            .validate("SOMEONE-ELSE")
            .expect_err("the body and the path must name the same account");

        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable());
    }

    #[test]
    fn zero_and_five_legs_are_both_refused() {
        for legs in [
            vec![],
            vec![leg("A"), leg("B"), leg("C"), leg("D"), leg("E")],
        ] {
            let error = request(legs)
                .validate("SENTINEL-5WX00042")
                .expect_err("the venue takes one to four legs");
            assert!(format!("{error}").contains("one to 4 legs"), "{error}");
        }
    }

    /// A repeated leg is almost always one leg written twice, and a doubled
    /// requirement is the kind of wrong that looks plausible.
    #[test]
    fn duplicate_legs_are_refused() {
        let error = request(vec![leg("AAPL"), leg("AAPL")])
            .validate("SENTINEL-5WX00042")
            .expect_err("duplicate legs must be refused");

        assert!(format!("{error}").contains("duplicates"), "{error}");
    }

    #[test]
    fn a_blank_symbol_is_refused() {
        assert!(
            request(vec![leg("   ")])
                .validate("SENTINEL-5WX00042")
                .is_err()
        );

        let mut blank_underlying = request(vec![leg("AAPL")]);
        blank_underlying.underlying_symbol = "  ".to_string();
        assert!(blank_underlying.validate("SENTINEL-5WX00042").is_err());
    }

    #[test]
    fn one_to_four_distinct_legs_are_accepted() {
        for count in 1..=MAX_MARGIN_LEGS {
            let legs: Vec<_> = (0..count).map(|i| leg(&format!("SYM{i}"))).collect();
            assert!(request(legs).validate("SENTINEL-5WX00042").is_ok());
        }
    }

    /// The body carries the fields the order type does not, which is why it is
    /// its own type rather than a serialised `Order`.
    #[test]
    fn the_request_serialises_the_fields_the_venue_requires() {
        let body = serde_json::to_value(request(vec![leg("AAPL")]).with_price(
            Decimal::from_f64(186.99).expect("a price"),
            PriceEffect::Debit,
        ))
        .expect("the request serialises");

        assert_eq!(body["account-number"], "SENTINEL-5WX00042");
        assert_eq!(body["underlying-symbol"], "AAPL");
        assert_eq!(body["underlying-instrument-type"], "Equity");
        assert_eq!(body["legs"][0]["action"], "Buy to Open");
        assert_eq!(body["price-effect"], "Debit");
    }

    /// An absent price is omitted rather than sent as null: a market order has
    /// no price, and `"price": null` is a different request from no price.
    #[test]
    fn an_absent_price_is_omitted_from_the_body() {
        // A market order, which is the case that actually carries no price.
        // The shared helper builds a `Limit`, and a limit with no price is now
        // refused before it is sent.
        let body = serde_json::to_value(MarginOrderRequest::new(
            "SENTINEL-5WX00042",
            "AAPL",
            InstrumentType::Equity,
            OrderType::Market,
            TimeInForce::Day,
            vec![leg("AAPL")],
        ))
        .expect("serialises");

        assert!(body.get("price").is_none(), "{body}");
        assert!(body.get("price-effect").is_none(), "{body}");
        assert!(body.get("stop-trigger").is_none(), "{body}");
        assert!(body.get("gtc-date").is_none(), "{body}");
        assert!(body.get("replaces-order-id").is_none(), "{body}");
        assert!(
            body["legs"][0].get("remaining-quantity").is_none(),
            "{body}"
        );
    }

    /// The estimate has to describe the order the caller means to place.
    ///
    /// The body the venue documents carries a trigger, an expiry day and a
    /// replacement identifier, and this type could represent none of them: a
    /// stop order was estimated with no trigger, a `GTD` order with no expiry,
    /// and a replacement as if it were new. Each of those is a different order
    /// from the one asked about, and the buying-power figure that comes back
    /// looks exactly as plausible.
    #[test]
    fn a_dated_stop_or_replacement_can_be_described_and_is_checked() {
        let day = NaiveDate::from_ymd_opt(2026, 12, 18).expect("a real day");
        let base = || {
            MarginOrderRequest::new(
                "SENTINEL-5WX00042",
                "AAPL",
                InstrumentType::Equity,
                OrderType::Stop,
                TimeInForce::Gtd,
                vec![leg("AAPL")],
            )
        };

        let complete = base()
            .with_stop_trigger(Decimal::from(180))
            .with_gtc_date(day)
            .replacing("order-991");
        complete
            .validate("SENTINEL-5WX00042")
            .expect("a fully described stop must be accepted");

        let body = serde_json::to_value(&complete).expect("must serialize");
        assert_eq!(body["stop-trigger"], serde_json::json!(180));
        assert_eq!(body["gtc-date"], serde_json::json!("2026-12-18"));
        assert_eq!(body["replaces-order-id"], serde_json::json!("order-991"));

        // A stop with no trigger is not a stop.
        assert!(
            base()
                .with_gtc_date(day)
                .validate("SENTINEL-5WX00042")
                .is_err()
        );
        // A GTD order with no day does not expire on any day.
        assert!(
            base()
                .with_stop_trigger(Decimal::from(180))
                .validate("SENTINEL-5WX00042")
                .is_err()
        );
        // And a trigger on an order type that has none.
        assert!(
            request(vec![leg("AAPL")])
                .with_stop_trigger(Decimal::from(180))
                .validate("SENTINEL-5WX00042")
                .is_err()
        );
    }

    /// A partially filled leg reserves margin for what is left of it.
    #[test]
    fn a_remaining_quantity_has_to_be_part_of_the_leg() {
        let with_remaining = |remaining: Decimal| {
            let mut only = leg("AAPL");
            only.quantity = Decimal::from(10);
            only.remaining_quantity = Some(remaining);
            request(vec![only])
        };

        with_remaining(Decimal::from(4))
            .validate("SENTINEL-5WX00042")
            .expect("four of ten is a part of it");
        assert!(
            with_remaining(Decimal::from(11))
                .validate("SENTINEL-5WX00042")
                .is_err()
        );
        assert!(
            with_remaining(Decimal::ZERO)
                .validate("SENTINEL-5WX00042")
                .is_err()
        );
    }

    /// Neither the report nor the request renders its account number.
    #[test]
    fn the_margin_records_render_without_the_account_number() {
        const ACCOUNT: &str = "SENTINEL-5WX00042";

        let body: serde_json::Value = serde_json::from_str(REQUIREMENTS).expect("valid JSON");
        let mut report: MarginRequirementsReport =
            serde_json::from_value(body["data"].clone()).expect("the report must decode");
        report.account_number = Some(AccountNumber(ACCOUNT.to_string()));

        let outgoing = request(vec![leg("AAPL")]);

        let rendered = format!("{report:?} {report} {outgoing:?} {outgoing}");
        assert!(!rendered.contains(ACCOUNT), "rendered: {rendered}");
        assert!(rendered.contains("{account}"), "{rendered}");
        // The figures themselves still render: they are what the caller asked
        // for, and the identifier is what they did not ask to have logged.
        assert!(rendered.contains("AAPL"), "{rendered}");

        // The request still serializes the real number: the venue requires it
        // in the body.
        let written = serde_json::to_string(&outgoing).expect("must serialize");
        assert!(written.contains(ACCOUNT));
    }

    /// The SPAN exchange is a closed set, so a typo cannot reach the venue.
    #[test]
    fn the_span_exchange_only_spells_what_the_contract_admits() {
        assert_eq!(SpanExchange::Cme.as_wire(), "CME");
        assert_eq!(SpanExchange::Cfe.as_wire(), "CFE");
        assert_eq!(SpanExchange::Cme.to_string(), "CME");
    }
}

// The records that carry an account number render through the redacting
// helper rather than the derives, which go via `Serialize` and would print it
// the moment anything wrote `{value:?}`. The nested rows come along: the
// helper walks the whole serialized value, so a report's groups and a dry
// run's legs cannot reintroduce the identifier.
crate::types::wire::redacted_account_render!(MarginRequirementsReport);
crate::types::wire::redacted_account_render!(MarginDryRunOrder);
crate::types::wire::redacted_account_render!(PositionLimit);
crate::types::wire::redacted_account_render!(MarginOrderRequest);
