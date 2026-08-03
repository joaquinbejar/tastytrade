/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 9/3/25
******************************************************************************/
use crate::PriceEffect;
use crate::accounts::AccountNumber;
use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents an account balance.
///
/// This struct holds various balance-related information for a trading account, including cash balance,
/// equity values for different asset classes (long and short positions), derivative values, futures values,
/// margin requirements, available funds, and various call values.  It's designed for deserialization
/// from a data source using `serde` with kebab-case renaming.  All numeric values are represented as
/// `Decimal` for precision.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Balance {
    /// The account number associated with this balance information.
    pub account_number: AccountNumber,

    /// The cash balance available in the account.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub cash_balance: Decimal,

    /// The total value of long equity positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_equity_value: Decimal,

    /// The total value of short equity positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_equity_value: Decimal,

    /// The total value of long derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_derivative_value: Decimal,

    /// The total value of short derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_derivative_value: Decimal,

    /// The total value of long futures positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_futures_value: Decimal,

    /// The total value of short futures positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_futures_value: Decimal,

    /// The total value of long futures derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_futures_derivative_value: Decimal,

    /// The total value of short futures derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_futures_derivative_value: Decimal,

    /// The total value of long marginable positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_margineable_value: Decimal,

    /// The total value of short marginable positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_margineable_value: Decimal,

    /// The margin equity.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub margin_equity: Decimal,

    /// The equity buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub equity_buying_power: Decimal,

    /// The derivative buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub derivative_buying_power: Decimal,

    /// The day trading buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trading_buying_power: Decimal,

    /// The futures margin requirement.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub futures_margin_requirement: Decimal,

    /// The available trading funds.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub available_trading_funds: Decimal,

    /// The maintenance requirement.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub maintenance_requirement: Decimal,

    /// The maintenance call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub maintenance_call_value: Decimal,

    /// The Reg T call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub reg_t_call_value: Decimal,

    /// The day trading call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trading_call_value: Decimal,

    /// The day equity call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_equity_call_value: Decimal,

    /// The net liquidating value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub net_liquidating_value: Decimal,

    /// The cash available to withdraw.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub cash_available_to_withdraw: Decimal,

    /// The day trade excess.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trade_excess: Decimal,

    /// The pending cash.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub pending_cash: Decimal,

    /// The pending cash effect.
    pub pending_cash_effect: PriceEffect,

    /// The pending margin interest.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub pending_margin_interest: Decimal,

    /// Effective cryptocurrency buying power
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub effective_cryptocurrency_buying_power: Decimal,

    // Everything below is `Option`. The published schema marks no balance
    // field required, and the venue genuinely omits some depending on what the
    // account is permitted to trade — a cash account has no futures margin. A
    // required field the venue skips would fail the whole decode, which on the
    // streaming path means an account balance notification silently becoming
    // an unreadable frame.
    //
    // `default` is not redundant next to `with`: an explicit `with` cancels
    // serde's implicit "absent Option is None", so a field that has both and
    // no `default` fails on absence.
    /// Margin equity as Apex recorded it at the start of the day.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub apex_starting_day_margin_equity: Option<Decimal>,

    /// Margin required against bond positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub bond_margin_requirement: Option<Decimal>,

    /// A manual adjustment applied to buying power.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub buying_power_adjustment: Option<Decimal>,

    /// Cash that has settled.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cash_settle_balance: Option<Decimal>,

    /// Balance available for closed-loop transfers.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub closed_loop_available_balance: Option<Decimal>,

    /// Margin required against cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cryptocurrency_margin_requirement: Option<Decimal>,

    /// Margin required against equity offerings.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub equity_offering_margin_requirement: Option<Decimal>,

    /// Margin required against fixed-income positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub fixed_income_security_margin_requirement: Option<Decimal>,

    /// Intraday margin required against futures.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub futures_intraday_margin_requirement: Option<Decimal>,

    /// Overnight margin required against futures.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub futures_overnight_margin_requirement: Option<Decimal>,

    /// Equities cash moving intraday.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub intraday_equities_cash_amount: Option<Decimal>,

    /// Futures cash moving intraday.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub intraday_futures_cash_amount: Option<Decimal>,

    /// Total value of long bond positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_bond_value: Option<Decimal>,

    /// Total value of long cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_cryptocurrency_value: Option<Decimal>,

    /// Total value of long fixed-income positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_fixed_income_security_value: Option<Decimal>,

    /// Total value of long index derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_index_derivative_value: Option<Decimal>,

    /// Equity above the maintenance requirement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub maintenance_excess: Option<Decimal>,

    /// Margin balance that has settled.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_settle_balance: Option<Decimal>,

    /// Previous day's cryptocurrency fiat movement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub previous_day_cryptocurrency_fiat_amount: Option<Decimal>,

    /// Margin required under Regulation T.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub reg_t_margin_requirement: Option<Decimal>,

    /// Total value of short cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_cryptocurrency_value: Option<Decimal>,

    /// Total value of short index derivative positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_index_derivative_value: Option<Decimal>,

    /// Equity option buying power from the special memorandum account.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub sma_equity_option_buying_power: Option<Decimal>,

    /// Apex's adjustment to the special memorandum account.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub special_memorandum_account_apex_adjustment: Option<Decimal>,

    /// The special memorandum account value.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub special_memorandum_account_value: Option<Decimal>,

    /// Liquidity pool rebate still pending.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub total_pending_liquidity_pool_rebate: Option<Decimal>,

    /// Total settled balance.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub total_settle_balance: Option<Decimal>,

    /// Cryptocurrency fiat movement that has not settled.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub unsettled_cryptocurrency_fiat_amount: Option<Decimal>,

    /// Derivative buying power already committed.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub used_derivative_buying_power: Option<Decimal>,

    /// Whether the buying-power adjustment is a debit or a credit.
    #[serde(default)]
    pub buying_power_adjustment_effect: Option<PriceEffect>,

    /// Whether the intraday equities cash is a debit or a credit.
    #[serde(default)]
    pub intraday_equities_cash_effect: Option<PriceEffect>,

    /// Whether the intraday futures cash is a debit or a credit.
    #[serde(default)]
    pub intraday_futures_cash_effect: Option<PriceEffect>,

    /// Whether the previous day's cryptocurrency fiat movement is a debit or a credit.
    #[serde(default)]
    pub previous_day_cryptocurrency_fiat_effect: Option<PriceEffect>,

    /// Whether the unsettled cryptocurrency fiat movement is a debit or a credit.
    #[serde(default)]
    pub unsettled_cryptocurrency_fiat_effect: Option<PriceEffect>,

    /// Calendar day the intraday equities cash becomes effective.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub intraday_equities_cash_effective_date: Option<NaiveDate>,

    /// Calendar day the intraday futures cash becomes effective.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub intraday_futures_cash_effective_date: Option<NaiveDate>,

    /// Calendar day the previous cryptocurrency fiat movement became effective.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub previous_date_cryptocurrency_fiat_effective_date: Option<NaiveDate>,

    /// Calendar day this balance describes.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub snapshot_date: Option<NaiveDate>,

    /// The currency these figures are denominated in, such as `USD`.
    #[serde(default)]
    pub currency: Option<String>,

    /// Which end of the trading day this balance describes.
    #[serde(default)]
    pub time_of_day: Option<SnapshotTimeOfDay>,
    /// The timestamp of the last balance update.
    #[serde(with = "crate::types::wire::datetime")]
    pub updated_at: DateTime<FixedOffset>,
}

/// Represents a snapshot of an account's balance at a specific point in time.
///
/// This struct is designed to be deserialized from a data source using `serde`,
/// with field names matching the `kebab-case` convention.  It provides a comprehensive
/// view of various balance components, including cash, equities, derivatives, futures,
/// and margin-related values.  All monetary values are represented using `Decimal`
/// for precision.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BalanceSnapshot {
    /// The account number associated with this balance snapshot.
    pub account_number: AccountNumber,
    /// The cash balance in the account.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub cash_balance: Decimal,
    /// The value of long equity positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_equity_value: Decimal,
    /// The value of short equity positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_equity_value: Decimal,
    /// The value of long derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_derivative_value: Decimal,
    /// The value of short derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_derivative_value: Decimal,
    /// The value of long futures positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_futures_value: Decimal,
    /// The value of short futures positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_futures_value: Decimal,
    /// The value of long futures derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_futures_derivative_value: Decimal,
    /// The value of short futures derivative positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_futures_derivative_value: Decimal,
    /// The value of long margineable positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub long_margineable_value: Decimal,
    /// The value of short margineable positions.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub short_margineable_value: Decimal,
    /// The margin equity.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub margin_equity: Decimal,
    /// The equity buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub equity_buying_power: Decimal,
    /// The derivative buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub derivative_buying_power: Decimal,
    /// The day trading buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trading_buying_power: Decimal,
    /// The futures margin requirement.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub futures_margin_requirement: Decimal,
    /// The available trading funds.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub available_trading_funds: Decimal,
    /// The maintenance requirement.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub maintenance_requirement: Decimal,
    /// The maintenance call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub maintenance_call_value: Decimal,
    /// The Reg T call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub reg_t_call_value: Decimal,
    /// The day trading call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trading_call_value: Decimal,
    /// The day equity call value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_equity_call_value: Decimal,
    /// The net liquidating value.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub net_liquidating_value: Decimal,
    /// The cash available to withdraw.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub cash_available_to_withdraw: Decimal,
    /// The day trade excess.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub day_trade_excess: Decimal,
    /// The pending cash.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub pending_cash: Decimal,
    /// The effect of pending cash on the account.
    pub pending_cash_effect: PriceEffect,
    /// The date of the snapshot.
    pub snapshot_date: chrono::NaiveDate,

    // Everything below was in the venue's schema and not in this struct, so a
    // caller reading a snapshot saw twenty-nine fewer numbers than arrived.
    // All `Option`: certification omits fields production sends, and a
    // balance that defaults to zero is a balance that lies.
    /// Margin required against bond positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub bond_margin_requirement: Option<Decimal>,
    /// Cash portion of the settlement balance.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cash_settle_balance: Option<Decimal>,
    /// Balance available through closed-loop transfer.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub closed_loop_available_balance: Option<Decimal>,
    /// Margin required against cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cryptocurrency_margin_requirement: Option<Decimal>,
    /// Which currency this row is denominated in.
    #[serde(default)]
    pub currency: Option<String>,
    /// Margin required against equity offerings.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub equity_offering_margin_requirement: Option<Decimal>,
    /// Margin required against fixed-income positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub fixed_income_security_margin_requirement: Option<Decimal>,
    /// Intraday equities cash movement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub intraday_equities_cash_amount: Option<Decimal>,
    /// Whether the intraday equities cash moved in or out.
    #[serde(default)]
    pub intraday_equities_cash_effect: Option<PriceEffect>,
    /// When the intraday equities cash settles.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub intraday_equities_cash_effective_date: Option<chrono::NaiveDate>,
    /// Intraday futures cash movement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub intraday_futures_cash_amount: Option<Decimal>,
    /// Whether the intraday futures cash moved in or out.
    #[serde(default)]
    pub intraday_futures_cash_effect: Option<PriceEffect>,
    /// When the intraday futures cash settles.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub intraday_futures_cash_effective_date: Option<chrono::NaiveDate>,
    /// Value of long bond positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_bond_value: Option<Decimal>,
    /// Value of long cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_cryptocurrency_value: Option<Decimal>,
    /// Value of long fixed-income positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub long_fixed_income_security_value: Option<Decimal>,
    /// Margin portion of the settlement balance.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub margin_settle_balance: Option<Decimal>,
    /// When the previous day's cryptocurrency fiat movement settles.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub previous_date_cryptocurrency_fiat_effective_date: Option<chrono::NaiveDate>,
    /// Previous day's cryptocurrency fiat movement.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub previous_day_cryptocurrency_fiat_amount: Option<Decimal>,
    /// Whether that movement was in or out.
    #[serde(default)]
    pub previous_day_cryptocurrency_fiat_effect: Option<PriceEffect>,
    /// Value of short cryptocurrency positions.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub short_cryptocurrency_value: Option<Decimal>,
    /// Equity-option buying power from the special memorandum account.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub sma_equity_option_buying_power: Option<Decimal>,
    /// Clearing-firm adjustment to the special memorandum account.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub special_memorandum_account_apex_adjustment: Option<Decimal>,
    /// Special memorandum account value.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub special_memorandum_account_value: Option<Decimal>,
    /// Whether this snapshot is beginning or end of day.
    #[serde(default)]
    pub time_of_day: Option<SnapshotTimeOfDay>,
    /// Total settlement balance.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub total_settle_balance: Option<Decimal>,
    /// Unsettled cryptocurrency fiat amount.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub unsettled_cryptocurrency_fiat_amount: Option<Decimal>,
    /// Whether the unsettled cryptocurrency fiat is in or out.
    #[serde(default)]
    pub unsettled_cryptocurrency_fiat_effect: Option<PriceEffect>,
    /// Derivative buying power already consumed.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub used_derivative_buying_power: Option<Decimal>,
}

/// Represents the time of day for a snapshot.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTimeOfDay {
    /// End of Day.
    #[serde(rename = "EOD")]
    Eod,
    /// Beginning of Day.
    #[serde(rename = "BOD")]
    Bod,
}

impl SnapshotTimeOfDay {
    /// The text the venue uses for this value.
    ///
    /// `EOD` and `BOD`, not `Eod` and `Bod`. The distinction is not cosmetic:
    /// `time-of-day` is a **required** query parameter on the snapshot
    /// endpoint, and this is what goes into it.
    pub fn as_wire(&self) -> &'static str {
        match self {
            SnapshotTimeOfDay::Eod => "EOD",
            SnapshotTimeOfDay::Bod => "BOD",
        }
    }
}

impl fmt::Display for SnapshotTimeOfDay {
    /// The venue's spelling.
    ///
    /// This used to be the derived `Debug` — `Eod` — and the snapshot query
    /// was built from it, so the required `time-of-day` parameter went out as
    /// `Eod` on every request. The serde rename was right all along and only
    /// the `Display` was wrong, which is why nothing that round-tripped
    /// through JSON ever noticed.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::AccountNumber;
    use chrono::Datelike;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    /// The regression: this test used to assert `"Eod"`, pinning the defect
    /// rather than the contract. The snapshot query is built from `Display`,
    /// so a required parameter was going out with a value the venue does not
    /// use.
    #[test]
    fn test_snapshot_time_of_day_display() {
        assert_eq!(format!("{}", SnapshotTimeOfDay::Eod), "EOD");
        assert_eq!(format!("{}", SnapshotTimeOfDay::Bod), "BOD");
        // And it agrees with what serde writes, which is the whole point.
        assert_eq!(
            serde_json::to_string(&SnapshotTimeOfDay::Eod).expect("serialises"),
            "\"EOD\""
        );
    }

    #[test]
    fn test_snapshot_time_of_day_serialization() {
        let eod = SnapshotTimeOfDay::Eod;
        let serialized = serde_json::to_string(&eod).unwrap();
        assert_eq!(serialized, "\"EOD\"");

        let bod = SnapshotTimeOfDay::Bod;
        let serialized = serde_json::to_string(&bod).unwrap();
        assert_eq!(serialized, "\"BOD\"");
    }

    #[test]
    fn test_snapshot_time_of_day_deserialization() {
        let eod: SnapshotTimeOfDay = serde_json::from_str("\"EOD\"").unwrap();
        matches!(eod, SnapshotTimeOfDay::Eod);

        let bod: SnapshotTimeOfDay = serde_json::from_str("\"BOD\"").unwrap();
        matches!(bod, SnapshotTimeOfDay::Bod);
    }

    #[test]
    fn test_balance_serialization() {
        let balance = Balance {
            account_number: AccountNumber("TEST123".to_string()),
            cash_balance: Decimal::from_str("1000.50").unwrap(),
            long_equity_value: Decimal::from_str("5000.00").unwrap(),
            short_equity_value: Decimal::from_str("0.00").unwrap(),
            long_derivative_value: Decimal::from_str("500.00").unwrap(),
            short_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_futures_value: Decimal::from_str("0.00").unwrap(),
            short_futures_value: Decimal::from_str("0.00").unwrap(),
            long_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            short_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_margineable_value: Decimal::from_str("5000.00").unwrap(),
            short_margineable_value: Decimal::from_str("0.00").unwrap(),
            margin_equity: Decimal::from_str("6500.50").unwrap(),
            equity_buying_power: Decimal::from_str("13000.00").unwrap(),
            derivative_buying_power: Decimal::from_str("6500.50").unwrap(),
            day_trading_buying_power: Decimal::from_str("26000.00").unwrap(),
            futures_margin_requirement: Decimal::from_str("0.00").unwrap(),
            available_trading_funds: Decimal::from_str("6500.50").unwrap(),
            maintenance_requirement: Decimal::from_str("0.00").unwrap(),
            maintenance_call_value: Decimal::from_str("0.00").unwrap(),
            reg_t_call_value: Decimal::from_str("0.00").unwrap(),
            day_trading_call_value: Decimal::from_str("0.00").unwrap(),
            day_equity_call_value: Decimal::from_str("0.00").unwrap(),
            net_liquidating_value: Decimal::from_str("6500.50").unwrap(),
            cash_available_to_withdraw: Decimal::from_str("1000.50").unwrap(),
            day_trade_excess: Decimal::from_str("26000.00").unwrap(),
            pending_cash: Decimal::from_str("0.00").unwrap(),
            pending_cash_effect: PriceEffect::None,
            pending_margin_interest: Decimal::from_str("0.00").unwrap(),
            effective_cryptocurrency_buying_power: Decimal::from_str("0.00").unwrap(),
            updated_at: DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z").unwrap(),
            apex_starting_day_margin_equity: None,
            bond_margin_requirement: None,
            buying_power_adjustment: None,
            cash_settle_balance: None,
            closed_loop_available_balance: None,
            cryptocurrency_margin_requirement: None,
            equity_offering_margin_requirement: None,
            fixed_income_security_margin_requirement: None,
            futures_intraday_margin_requirement: None,
            futures_overnight_margin_requirement: None,
            intraday_equities_cash_amount: None,
            intraday_futures_cash_amount: None,
            long_bond_value: None,
            long_cryptocurrency_value: None,
            long_fixed_income_security_value: None,
            long_index_derivative_value: None,
            maintenance_excess: None,
            margin_settle_balance: None,
            previous_day_cryptocurrency_fiat_amount: None,
            reg_t_margin_requirement: None,
            short_cryptocurrency_value: None,
            short_index_derivative_value: None,
            sma_equity_option_buying_power: None,
            special_memorandum_account_apex_adjustment: None,
            special_memorandum_account_value: None,
            total_pending_liquidity_pool_rebate: None,
            total_settle_balance: None,
            unsettled_cryptocurrency_fiat_amount: None,
            used_derivative_buying_power: None,
            buying_power_adjustment_effect: None,
            intraday_equities_cash_effect: None,
            intraday_futures_cash_effect: None,
            previous_day_cryptocurrency_fiat_effect: None,
            unsettled_cryptocurrency_fiat_effect: None,
            intraday_equities_cash_effective_date: None,
            intraday_futures_cash_effective_date: None,
            previous_date_cryptocurrency_fiat_effective_date: None,
            snapshot_date: None,
            currency: None,
            time_of_day: None,
        };

        let serialized = serde_json::to_string(&balance).unwrap();
        assert!(serialized.contains("TEST123"));
        assert!(serialized.contains("1000.50"));
        assert!(serialized.contains("5000.00"));
        assert!(serialized.contains("None"));
    }

    #[test]
    fn test_balance_snapshot_creation() {
        let snapshot = BalanceSnapshot {
            account_number: AccountNumber("SNAP123".to_string()),
            cash_balance: Decimal::from_str("2000.00").unwrap(),
            long_equity_value: Decimal::from_str("8000.00").unwrap(),
            short_equity_value: Decimal::from_str("0.00").unwrap(),
            long_derivative_value: Decimal::from_str("1000.00").unwrap(),
            short_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_futures_value: Decimal::from_str("0.00").unwrap(),
            short_futures_value: Decimal::from_str("0.00").unwrap(),
            long_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            short_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_margineable_value: Decimal::from_str("8000.00").unwrap(),
            short_margineable_value: Decimal::from_str("0.00").unwrap(),
            margin_equity: Decimal::from_str("11000.00").unwrap(),
            equity_buying_power: Decimal::from_str("22000.00").unwrap(),
            derivative_buying_power: Decimal::from_str("11000.00").unwrap(),
            day_trading_buying_power: Decimal::from_str("44000.00").unwrap(),
            futures_margin_requirement: Decimal::from_str("0.00").unwrap(),
            available_trading_funds: Decimal::from_str("11000.00").unwrap(),
            maintenance_requirement: Decimal::from_str("0.00").unwrap(),
            maintenance_call_value: Decimal::from_str("0.00").unwrap(),
            reg_t_call_value: Decimal::from_str("0.00").unwrap(),
            day_trading_call_value: Decimal::from_str("0.00").unwrap(),
            day_equity_call_value: Decimal::from_str("0.00").unwrap(),
            net_liquidating_value: Decimal::from_str("11000.00").unwrap(),
            cash_available_to_withdraw: Decimal::from_str("2000.00").unwrap(),
            day_trade_excess: Decimal::from_str("44000.00").unwrap(),
            pending_cash: Decimal::from_str("0.00").unwrap(),
            pending_cash_effect: PriceEffect::Credit,
            snapshot_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            // The fields this struct gained from the venue's schema. `None`
            // rather than zero: a balance nobody reported is not a balance of
            // nothing.
            bond_margin_requirement: None,
            cash_settle_balance: None,
            closed_loop_available_balance: None,
            cryptocurrency_margin_requirement: None,
            currency: None,
            equity_offering_margin_requirement: None,
            fixed_income_security_margin_requirement: None,
            intraday_equities_cash_amount: None,
            intraday_equities_cash_effect: None,
            intraday_equities_cash_effective_date: None,
            intraday_futures_cash_amount: None,
            intraday_futures_cash_effect: None,
            intraday_futures_cash_effective_date: None,
            long_bond_value: None,
            long_cryptocurrency_value: None,
            long_fixed_income_security_value: None,
            margin_settle_balance: None,
            previous_date_cryptocurrency_fiat_effective_date: None,
            previous_day_cryptocurrency_fiat_amount: None,
            previous_day_cryptocurrency_fiat_effect: None,
            short_cryptocurrency_value: None,
            sma_equity_option_buying_power: None,
            special_memorandum_account_apex_adjustment: None,
            special_memorandum_account_value: None,
            time_of_day: None,
            total_settle_balance: None,
            unsettled_cryptocurrency_fiat_amount: None,
            unsettled_cryptocurrency_fiat_effect: None,
            used_derivative_buying_power: None,
        };

        assert_eq!(snapshot.account_number.0, "SNAP123");
        assert_eq!(snapshot.cash_balance, Decimal::from_str("2000.00").unwrap());
        assert_eq!(snapshot.snapshot_date.year(), 2024);
        matches!(snapshot.pending_cash_effect, PriceEffect::Credit);
    }

    #[test]
    fn test_balance_debug_format() {
        let balance = Balance {
            account_number: AccountNumber("DEBUG123".to_string()),
            cash_balance: Decimal::from_str("100.00").unwrap(),
            long_equity_value: Decimal::from_str("500.00").unwrap(),
            short_equity_value: Decimal::from_str("0.00").unwrap(),
            long_derivative_value: Decimal::from_str("0.00").unwrap(),
            short_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_futures_value: Decimal::from_str("0.00").unwrap(),
            short_futures_value: Decimal::from_str("0.00").unwrap(),
            long_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            short_futures_derivative_value: Decimal::from_str("0.00").unwrap(),
            long_margineable_value: Decimal::from_str("500.00").unwrap(),
            short_margineable_value: Decimal::from_str("0.00").unwrap(),
            margin_equity: Decimal::from_str("600.00").unwrap(),
            equity_buying_power: Decimal::from_str("1200.00").unwrap(),
            derivative_buying_power: Decimal::from_str("600.00").unwrap(),
            day_trading_buying_power: Decimal::from_str("2400.00").unwrap(),
            futures_margin_requirement: Decimal::from_str("0.00").unwrap(),
            available_trading_funds: Decimal::from_str("600.00").unwrap(),
            maintenance_requirement: Decimal::from_str("0.00").unwrap(),
            maintenance_call_value: Decimal::from_str("0.00").unwrap(),
            reg_t_call_value: Decimal::from_str("0.00").unwrap(),
            day_trading_call_value: Decimal::from_str("0.00").unwrap(),
            day_equity_call_value: Decimal::from_str("0.00").unwrap(),
            net_liquidating_value: Decimal::from_str("600.00").unwrap(),
            cash_available_to_withdraw: Decimal::from_str("100.00").unwrap(),
            day_trade_excess: Decimal::from_str("2400.00").unwrap(),
            pending_cash: Decimal::from_str("0.00").unwrap(),
            pending_cash_effect: PriceEffect::Debit,
            pending_margin_interest: Decimal::from_str("0.00").unwrap(),
            effective_cryptocurrency_buying_power: Decimal::from_str("0.00").unwrap(),
            updated_at: DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z").unwrap(),
            apex_starting_day_margin_equity: None,
            bond_margin_requirement: None,
            buying_power_adjustment: None,
            cash_settle_balance: None,
            closed_loop_available_balance: None,
            cryptocurrency_margin_requirement: None,
            equity_offering_margin_requirement: None,
            fixed_income_security_margin_requirement: None,
            futures_intraday_margin_requirement: None,
            futures_overnight_margin_requirement: None,
            intraday_equities_cash_amount: None,
            intraday_futures_cash_amount: None,
            long_bond_value: None,
            long_cryptocurrency_value: None,
            long_fixed_income_security_value: None,
            long_index_derivative_value: None,
            maintenance_excess: None,
            margin_settle_balance: None,
            previous_day_cryptocurrency_fiat_amount: None,
            reg_t_margin_requirement: None,
            short_cryptocurrency_value: None,
            short_index_derivative_value: None,
            sma_equity_option_buying_power: None,
            special_memorandum_account_apex_adjustment: None,
            special_memorandum_account_value: None,
            total_pending_liquidity_pool_rebate: None,
            total_settle_balance: None,
            unsettled_cryptocurrency_fiat_amount: None,
            used_derivative_buying_power: None,
            buying_power_adjustment_effect: None,
            intraday_equities_cash_effect: None,
            intraday_futures_cash_effect: None,
            previous_day_cryptocurrency_fiat_effect: None,
            unsettled_cryptocurrency_fiat_effect: None,
            intraday_equities_cash_effective_date: None,
            intraday_futures_cash_effective_date: None,
            previous_date_cryptocurrency_fiat_effective_date: None,
            snapshot_date: None,
            currency: None,
            time_of_day: None,
        };

        let debug_str = format!("{:?}", balance);
        assert!(debug_str.contains("DEBUG123"));
    }
}
