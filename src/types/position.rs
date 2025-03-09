use super::order::{PriceEffect, Symbol};
use crate::accounts::AccountNumber;
use crate::types::instrument::InstrumentType;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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
