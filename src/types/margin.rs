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

use crate::types::instrument::InstrumentType;
use crate::types::order::{Action, OrderType, PriceEffect, TimeInForce};

/// The account's current margin and capital requirements, grouped by
/// underlying.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginRequirementsReport {
    /// Which account this is about. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
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
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginDryRunOrder {
    /// The venue's placeholder identifier, e.g. `dry-run-0`.
    #[serde(default)]
    pub id: Option<String>,
    /// Which account it would be placed against. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
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
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PositionLimit {
    /// The venue's identifier for this limit record.
    #[serde(default)]
    pub id: Option<i64>,
    /// Which account this is about. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
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

/// An order to estimate margin for.
///
/// Deliberately **not** [`crate::prelude::Order`]. The margin endpoint requires
/// an account number and an underlying symbol that the order type does not
/// carry, and serialising an `Order` into this body would send a request
/// missing two required fields. It also cannot route: there is no path from
/// here to a placement.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MarginOrderRequest {
    /// Which account to estimate against.
    pub account_number: String,
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
}

/// The most legs the venue accepts on a margin estimate.
pub const MAX_MARGIN_LEGS: usize = 4;

impl MarginOrderRequest {
    /// A request for `account_number` on `underlying_symbol`.
    pub fn new(
        account_number: impl Into<String>,
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

    /// Fails when the request cannot be what the venue accepts.
    ///
    /// Local checks only, all of them
    /// [`crate::TastyTradeError::Precondition`] and therefore not retryable:
    /// nothing was sent, and sending it again would fail the same way.
    pub(crate) fn validate(&self, account_number: &str) -> crate::TastyResult<()> {
        if self.account_number != account_number {
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
        }
    }

    fn request(legs: Vec<MarginOrderLeg>) -> MarginOrderRequest {
        MarginOrderRequest::new(
            "SENTINEL-5WX00042",
            "AAPL",
            InstrumentType::Equity,
            OrderType::Limit,
            TimeInForce::Day,
            legs,
        )
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
        let body = serde_json::to_value(request(vec![leg("AAPL")])).expect("serialises");

        assert!(body.get("price").is_none(), "{body}");
        assert!(body.get("price-effect").is_none(), "{body}");
    }
}
