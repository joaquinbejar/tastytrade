use super::order::Symbol;
use crate::api::quote_streaming::DxFeedSymbol;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InstrumentType {
    Equity,
    #[serde(rename = "Equity Option")]
    EquityOption,
    #[serde(rename = "Equity Offering")]
    EquityOffering,
    Future,
    #[serde(rename = "Future Option")]
    FutureOption,
    Cryptocurrency,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityInstrumentInfo {
    pub symbol: Symbol,
    pub streamer_symbol: DxFeedSymbol,
}

/// Estructura para los tamaños de tick
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TickSize {
    pub value: String,
    pub threshold: Option<String>,
}

/// Estructura para un instrumento de tipo Equity
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityInstrument {
    pub id: u64,
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub cusip: Option<String>,
    pub short_description: String,
    pub is_index: bool,
    pub listed_market: String,
    pub description: String,
    pub lendability: Option<String>,
    pub borrow_rate: Option<String>,
    pub market_time_instrument_collection: String,
    pub is_closing_only: bool,
    pub is_options_closing_only: bool,
    pub active: bool,
    pub is_fractional_quantity_eligible: bool,
    pub is_illiquid: bool,
    pub is_etf: bool,
    pub streamer_symbol: DxFeedSymbol,
    pub tick_sizes: Option<Vec<TickSize>>,
    pub option_tick_sizes: Option<Vec<TickSize>>,
}

/// Estructura para los strikes en cadenas de opciones
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Strike {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    pub call: Symbol,
    pub call_streamer_symbol: DxFeedSymbol,
    pub put: Symbol,
    pub put_streamer_symbol: DxFeedSymbol,
}

/// Estructura para las expiraciones de opciones
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Expiration {
    pub expiration_type: String,
    pub expiration_date: String,
    pub days_to_expiration: u64,
    pub settlement_type: String,
    pub strikes: Vec<Strike>,
}

/// Estructura para cadenas de opciones anidadas
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NestedOptionChain {
    pub underlying_symbol: Symbol,
    pub root_symbol: Symbol,
    pub option_chain_type: String,
    pub shares_per_contract: u64,
    pub expirations: Vec<Expiration>,
}

/// Estructura para opciones de acciones
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityOption {
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub active: bool,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    pub root_symbol: Symbol,
    pub underlying_symbol: Symbol,
    pub expiration_date: String,
    pub exercise_style: String,
    pub shares_per_contract: u64,
    pub option_type: String,
    pub option_chain_type: String,
    pub expiration_type: String,
    pub settlement_type: String,
    pub stops_trading_at: String,
    pub market_time_instrument_collection: String,
    pub days_to_expiration: u64,
    pub expires_at: String,
    pub is_closing_only: bool,
    pub streamer_symbol: DxFeedSymbol,
}

/// Estructura para contratos de futuros
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Future {
    pub symbol: Symbol,
    pub product_code: String,
    pub contract_size: String,
    pub tick_size: String,
    pub notional_multiplier: String,
    pub main_fraction: String,
    pub sub_fraction: String,
    pub display_factor: String,
    pub last_trade_date: String,
    pub expiration_date: String,
    pub closing_only_date: String,
    pub active: bool,
    pub active_month: bool,
    pub next_active_month: bool,
    pub is_closing_only: bool,
    pub stops_trading_at: String,
    pub expires_at: String,
    pub product_group: String,
    pub exchange: String,
    pub roll_target_symbol: Option<Symbol>,
    pub streamer_exchange_code: String,
    pub streamer_symbol: DxFeedSymbol,
    pub back_month_first_calendar_symbol: bool,
    pub is_tradeable: bool,
    pub future_product: FutureProduct,
    pub tick_sizes: Vec<TickSize>,
    pub option_tick_sizes: Vec<TickSize>,
    pub spread_tick_sizes: Option<Vec<HashMap<String, String>>>,
}

/// Estructura para productos de futuros
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureProduct {
    pub root_symbol: Symbol,
    pub code: String,
    pub description: String,
    pub clearing_code: String,
    pub clearing_exchange_code: String,
    pub clearport_code: String,
    pub legacy_code: String,
    pub exchange: String,
    pub legacy_exchange_code: String,
    pub product_type: String,
    pub listed_months: Vec<String>,
    pub active_months: Vec<String>,
    pub notional_multiplier: String,
    pub tick_size: String,
    pub display_factor: String,
    pub streamer_exchange_code: String,
    pub small_notional: bool,
    pub back_month_first_calendar_symbol: bool,
    pub first_notice: bool,
    pub cash_settled: bool,
    pub security_group: String,
    pub market_sector: String,
    pub roll: FutureRoll,
}

/// Estructura para roll de futuros
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureRoll {
    pub name: String,
    pub active_count: u32,
    pub cash_settled: bool,
    pub business_days_offset: u32,
    pub first_notice: bool,
}

/// Estructura para opciones de futuros
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureOption {
    pub symbol: Symbol,
    pub underlying_symbol: Symbol,
    pub product_code: String,
    pub expiration_date: String,
    pub root_symbol: Symbol,
    pub option_root_symbol: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    pub exchange: String,
    pub exchange_symbol: String,
    pub streamer_symbol: DxFeedSymbol,
    pub option_type: String,
    pub exercise_style: String,
    pub is_vanilla: bool,
    pub is_primary_deliverable: bool,
    pub future_price_ratio: String,
    pub multiplier: String,
    pub underlying_count: String,
    pub is_confirmed: bool,
    pub notional_value: String,
    pub display_factor: String,
    pub security_exchange: String,
    pub sx_id: String,
    pub settlement_type: String,
    pub strike_factor: String,
    pub maturity_date: String,
    pub is_exercisable_weekly: bool,
    pub last_trade_time: String,
    pub days_to_expiration: i32,
    pub is_closing_only: bool,
    pub active: bool,
    pub stops_trading_at: String,
    pub expires_at: String,
    pub future_option_product: FutureOptionProduct,
}

/// Estructura para productos de opciones de futuros
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureOptionProduct {
    pub root_symbol: String,
    pub cash_settled: bool,
    pub code: String,
    pub legacy_code: String,
    pub clearport_code: String,
    pub clearing_code: String,
    pub clearing_exchange_code: String,
    pub clearing_price_multiplier: String,
    pub display_factor: String,
    pub exchange: String,
    pub product_type: String,
    pub expiration_type: String,
    pub settlement_delay_days: u32,
    pub is_rollover: bool,
    pub market_sector: String,
}

/// Estructura para criptomonedas
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Cryptocurrency {
    pub id: u64,
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub short_description: String,
    pub description: String,
    pub is_closing_only: bool,
    pub active: bool,
    pub tick_size: String,
    pub streamer_symbol: DxFeedSymbol,
    pub destination_venue_symbols: Vec<DestinationVenueSymbol>,
}

/// Estructura para símbolos de venue de destino
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DestinationVenueSymbol {
    pub id: u64,
    pub symbol: Symbol,
    pub destination_venue: String,
    pub max_quantity_precision: u32,
    pub max_price_precision: u32,
    pub routable: bool,
}

/// Estructura para warrants
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Warrant {
    pub symbol: Symbol,
    pub instrument_type: InstrumentType,
    pub listed_market: String,
    pub description: String,
    pub is_closing_only: bool,
    pub active: bool,
}

/// Estructura para precisiones decimales de cantidad
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuantityDecimalPrecision {
    pub instrument_type: InstrumentType,
    pub symbol: Option<Symbol>,
    pub value: u32,
    pub minimum_increment_precision: u32,
}
