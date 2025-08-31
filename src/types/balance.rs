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
use pretty_simple_display::{DebugPretty, DisplaySimple};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::AccountNumber;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use chrono::Datelike;

    #[test]
    fn test_snapshot_time_of_day_display() {
        assert_eq!(format!("{}", SnapshotTimeOfDay::Eod), "Eod");
        assert_eq!(format!("{}", SnapshotTimeOfDay::Bod), "Bod");
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
            updated_at: "2024-01-01T12:00:00Z".to_string(),
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
            updated_at: "2024-01-01T12:00:00Z".to_string(),
        };
        
        let debug_str = format!("{:?}", balance);
        assert!(debug_str.contains("DEBUG123"));
        assert!(debug_str.contains("Balance"));
    }
}
