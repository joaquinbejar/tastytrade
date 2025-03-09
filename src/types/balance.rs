/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 9/3/25
******************************************************************************/
use crate::PriceEffect;
use crate::accounts::AccountNumber;
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
#[derive(Debug, Serialize, Deserialize)]
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

    /// The timestamp of the last balance update.
    pub updated_at: String,
}

/// Represents a snapshot of an account's balance at a specific point in time.
///
/// This struct is designed to be deserialized from a data source using `serde`,
/// with field names matching the `kebab-case` convention.  It provides a comprehensive
/// view of various balance components, including cash, equities, derivatives, futures,
/// and margin-related values.  All monetary values are represented using `Decimal`
/// for precision.
#[derive(Debug, Serialize, Deserialize)]
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
}

/// Represents the time of day for a snapshot.
#[derive(Debug, Serialize, Deserialize)]
pub enum SnapshotTimeOfDay {
    /// End of Day.
    #[serde(rename = "EOD")]
    Eod,
    /// Beginning of Day.
    #[serde(rename = "BOD")]
    Bod,
}

impl fmt::Display for SnapshotTimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
