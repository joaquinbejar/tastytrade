use super::order::{PriceEffect, Symbol};
use crate::accounts::AccountNumber;
use crate::types::instrument::InstrumentType;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use pretty_simple_display::{DebugPretty, DisplaySimple};

/// Represents the direction of a quantity, such as a trade or position.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum QuantityDirection {
    /// Represents a long position or buy trade.
    Long,
    /// Represents a short position or sell trade.
    Short,
    /// Represents a zero quantity or a neutral position.
    Zero,
}

impl Display for QuantityDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuantityDirection::Long => write!(f, "Long"),
            QuantityDirection::Short => write!(f, "Short"),
            QuantityDirection::Zero => write!(f, "Zero"),
        }
    }
}

/// Represents a full position for an account.
///
/// This struct provides detailed information about a specific position held in an account, including
/// the instrument, quantity, price details, and various flags.  It's designed for deserialization
/// with kebab-case renaming for compatibility with external APIs.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FullPosition {
    /// The account number associated with the position.
    pub account_number: AccountNumber,
    /// The symbol of the instrument for this position.
    pub symbol: Symbol,
    /// The type of the instrument (e.g., Equity, Option).
    pub instrument_type: InstrumentType,
    /// The underlying symbol of the instrument, if applicable (e.g., for options).
    pub underlying_symbol: Symbol,
    /// The quantity of the instrument held in the position.  Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub quantity: Decimal,
    /// The direction of the quantity (Long, Short, or Zero).
    pub quantity_direction: QuantityDirection,
    /// The closing price of the instrument.  Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub close_price: Decimal,
    /// The average opening price of the instrument. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_open_price: Decimal,
    /// The average yearly market close price of the instrument. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_yearly_market_close_price: Decimal,
    /// The average daily market close price of the instrument. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_daily_market_close_price: Decimal,
    /// The multiplier for the instrument. Uses floating-point deserialization for the Decimal type.
    #[serde(with = "rust_decimal::serde::float")]
    pub multiplier: Decimal,
    /// The effect of the price on the account (Debit, Credit, or None).
    pub cost_effect: PriceEffect,
    /// A flag indicating whether the position is suppressed.
    pub is_suppressed: bool,
    /// A flag indicating whether the position is frozen.
    pub is_frozen: bool,
    /// The restricted quantity of the instrument. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub restricted_quantity: Decimal,
    /// The realized day gain for the position. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_day_gain: Decimal,
    /// The effect of the realized day gain (e.g., "Debit", "Credit").
    pub realized_day_gain_effect: String,
    /// The date of the realized day gain.
    pub realized_day_gain_date: String,
    /// The realized gain for today. Uses arbitrary precision for accuracy.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_today: Decimal,
    /// The effect of the realized gain for today (e.g., "Debit", "Credit").
    pub realized_today_effect: String,
    /// The date of the realized gain for today.
    pub realized_today_date: String,
    /// The date and time when the position was created.
    pub created_at: String,
    /// The date and time when the position was last updated.
    pub updated_at: String,
}

/// Represents a brief overview of a position.
///
/// This struct provides a summary of a trading position, including details such as
/// the account number, symbol, quantity, price, and various status flags.  It's
/// designed for deserialization with kebab-case renaming for compatibility with
/// external APIs.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BriefPosition {
    /// The account number associated with the position.
    pub account_number: AccountNumber,
    /// The trading symbol of the instrument.
    pub symbol: Symbol,
    /// The type of the instrument (e.g., Equity, Option).
    pub instrument_type: InstrumentType,
    /// The underlying symbol of the instrument (if applicable).
    pub underlying_symbol: Symbol,
    /// The quantity of the instrument held in the position.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub quantity: Decimal,
    /// The direction of the quantity (Long, Short, or Zero).
    pub quantity_direction: QuantityDirection,
    /// The closing price of the instrument.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub close_price: Decimal,
    /// The average opening price of the instrument.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_open_price: Decimal,
    /// The multiplier for the instrument.
    #[serde(with = "rust_decimal::serde::float")]
    pub multiplier: Decimal,
    /// The effect of the price on the account (Debit, Credit, or None).
    pub cost_effect: PriceEffect,
    /// A flag indicating whether the position is suppressed.
    pub is_suppressed: bool,
    /// A flag indicating whether the position is frozen.
    pub is_frozen: bool,
    /// The restricted quantity of the instrument.
    #[serde(with = "rust_decimal::serde::float")]
    pub restricted_quantity: Decimal,
    /// The realized day gain for the position.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_day_gain: Decimal,
    /// The realized amount for today.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_today: Decimal,
    /// The timestamp of when the position was created.
    pub created_at: String,
    /// The timestamp of when the position was last updated.
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_quantity_direction_display() {
        assert_eq!(format!("{}", QuantityDirection::Long), "Long");
        assert_eq!(format!("{}", QuantityDirection::Short), "Short");
        assert_eq!(format!("{}", QuantityDirection::Zero), "Zero");
    }

    #[test]
    fn test_quantity_direction_serialization() {
        let long = QuantityDirection::Long;
        let serialized = serde_json::to_string(&long).unwrap();
        assert_eq!(serialized, "\"Long\"");
        
        let short = QuantityDirection::Short;
        let serialized = serde_json::to_string(&short).unwrap();
        assert_eq!(serialized, "\"Short\"");
        
        let zero = QuantityDirection::Zero;
        let serialized = serde_json::to_string(&zero).unwrap();
        assert_eq!(serialized, "\"Zero\"");
    }

    #[test]
    fn test_quantity_direction_deserialization() {
        let long: QuantityDirection = serde_json::from_str("\"Long\"").unwrap();
        matches!(long, QuantityDirection::Long);
        
        let short: QuantityDirection = serde_json::from_str("\"Short\"").unwrap();
        matches!(short, QuantityDirection::Short);
        
        let zero: QuantityDirection = serde_json::from_str("\"Zero\"").unwrap();
        matches!(zero, QuantityDirection::Zero);
    }

    #[test]
    fn test_quantity_direction_clone_and_copy() {
        let original = QuantityDirection::Long;
        let cloned = original.clone();
        let copied = original;
        
        matches!(cloned, QuantityDirection::Long);
        matches!(copied, QuantityDirection::Long);
    }

    #[test]
    fn test_quantity_direction_debug() {
        let long = QuantityDirection::Long;
        let debug_str = format!("{:?}", long);
        assert_eq!(debug_str, "Long");
    }

    #[test]
    fn test_full_position_debug() {
        // We can't easily create a FullPosition due to all the required fields,
        // but we can test that the struct exists and has the expected fields
        // by checking if we can deserialize a minimal JSON
        let json = r#"{
            "account-number": "TEST123",
            "symbol": "AAPL",
            "instrument-type": "Equity",
            "underlying-symbol": "AAPL",
            "quantity": "100",
            "quantity-direction": "Long",
            "close-price": "150.50",
            "average-open-price": "145.00",
            "average-yearly-market-close-price": "140.00",
            "average-daily-market-close-price": "149.00",
            "multiplier": 1.0,
            "cost-effect": "Debit",
            "is-suppressed": false,
            "is-frozen": false,
            "restricted-quantity": "0",
            "realized-day-gain": "550.00",
            "realized-day-gain-effect": "Credit",
            "realized-day-gain-date": "2024-01-01",
            "realized-today": "550.00",
            "realized-today-effect": "Credit",
            "realized-today-date": "2024-01-01",
            "created-at": "2024-01-01T10:00:00Z",
            "updated-at": "2024-01-01T16:00:00Z"
        }"#;
        
        let position: Result<FullPosition, _> = serde_json::from_str(json);
        assert!(position.is_ok());
        
        let position = position.unwrap();
        assert_eq!(position.account_number.0, "TEST123");
        assert_eq!(position.symbol.0, "AAPL");
        assert_eq!(position.quantity, Decimal::from_str("100").unwrap());
        matches!(position.quantity_direction, QuantityDirection::Long);
        matches!(position.instrument_type, InstrumentType::Equity);
    }

    #[test]
    fn test_brief_position_debug() {
        let json = r#"{
            "account-number": "BRIEF123",
            "symbol": "MSFT",
            "instrument-type": "Equity",
            "underlying-symbol": "MSFT",
            "quantity": "50",
            "quantity-direction": "Short",
            "close-price": "300.00",
            "average-open-price": "295.00",
            "multiplier": 1.0,
            "cost-effect": "Credit",
            "is-suppressed": true,
            "is-frozen": false,
            "restricted-quantity": 10.0,
            "realized-day-gain": "-250.00",
            "realized-today": "-250.00",
            "created-at": "2024-01-01T09:00:00Z",
            "updated-at": "2024-01-01T15:30:00Z"
        }"#;
        
        let position: Result<BriefPosition, _> = serde_json::from_str(json);
        assert!(position.is_ok());
        
        let position = position.unwrap();
        assert_eq!(position.account_number.0, "BRIEF123");
        assert_eq!(position.symbol.0, "MSFT");
        assert_eq!(position.quantity, Decimal::from_str("50").unwrap());
        matches!(position.quantity_direction, QuantityDirection::Short);
        assert!(position.is_suppressed);
        assert!(!position.is_frozen);
    }

    #[test]
    fn test_position_with_zero_quantity() {
        let json = r#"{
            "account-number": "ZERO123",
            "symbol": "TSLA",
            "instrument-type": "Equity",
            "underlying-symbol": "TSLA",
            "quantity": "0",
            "quantity-direction": "Zero",
            "close-price": "200.00",
            "average-open-price": "200.00",
            "multiplier": 1.0,
            "cost-effect": "None",
            "is-suppressed": false,
            "is-frozen": false,
            "restricted-quantity": 0.0,
            "realized-day-gain": "0.00",
            "realized-today": "0.00",
            "created-at": "2024-01-01T12:00:00Z",
            "updated-at": "2024-01-01T12:00:00Z"
        }"#;
        
        let position: Result<BriefPosition, _> = serde_json::from_str(json);
        assert!(position.is_ok());
        
        let position = position.unwrap();
        matches!(position.quantity_direction, QuantityDirection::Zero);
        assert_eq!(position.quantity, Decimal::ZERO);
        matches!(position.cost_effect, PriceEffect::None);
    }
}
