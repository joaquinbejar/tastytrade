use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::accounts::AccountNumber;

use super::order::{InstrumentType, PriceEffect, Symbol};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum QuantityDirection {
    Long,
    Short,
    Zero,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FullPosition {
    pub account_number: AccountNumber,
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub underlying_symbol: Symbol,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub quantity: Decimal,
    pub quantity_direction: QuantityDirection,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub close_price: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_open_price: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_yearly_market_close_price: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_daily_market_close_price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub multiplier: Decimal,
    pub cost_effect: PriceEffect,
    pub is_suppressed: bool,
    pub is_frozen: bool,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub restricted_quantity: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_day_gain: Decimal,
    pub realized_day_gain_effect: String,
    pub realized_day_gain_date: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_today: Decimal,
    pub realized_today_effect: String,
    pub realized_today_date: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BriefPosition {
    pub account_number: AccountNumber,
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub underlying_symbol: Symbol,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub quantity: Decimal,
    pub quantity_direction: QuantityDirection,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub close_price: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub average_open_price: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub multiplier: Decimal,
    pub cost_effect: PriceEffect,
    pub is_suppressed: bool,
    pub is_frozen: bool,
    #[serde(with = "rust_decimal::serde::float")]
    pub restricted_quantity: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_day_gain: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub realized_today: Decimal,
    pub created_at: String,
    pub updated_at: String,
}
