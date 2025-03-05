use derive_builder::Builder;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::accounts::AccountNumber;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PriceEffect {
    Debit,
    Credit,
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Action {
    #[serde(rename = "Buy to Open")]
    BuyToOpen,
    #[serde(rename = "Sell to Open")]
    SellToOpen,
    #[serde(rename = "Buy to Close")]
    BuyToClose,
    #[serde(rename = "Sell to Close")]
    SellToClose,
    Sell,
    Buy,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OrderType {
    Limit,
    Market,
    #[serde(rename = "Marketable Limit")]
    MarketableLimit,
    Stop,
    #[serde(rename = "Stop Limit")]
    StopLimit,
    #[serde(rename = "Notional Market")]
    NotionalMarket,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TimeInForce {
    Day,
    GTC,
    GTD,
    Ext,
    #[serde(rename = "GTC Ext")]
    GTCExt,
    IOC,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OrderStatus {
    Received,
    Routed,
    #[serde(rename = "In Flight")]
    InFlight,
    Live,
    #[serde(rename = "Cancel Requested")]
    CancelRequested,
    #[serde(rename = "Replace Requested")]
    ReplaceRequested,
    Contingent,
    Filled,
    Cancelled,
    Expired,
    Rejected,
    Removed,
    #[serde(rename = "Partially Removed")]
    PartiallyRemoved,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct Symbol(pub String);

impl<T: AsRef<str>> From<T> for Symbol {
    fn from(value: T) -> Self {
        Self(value.as_ref().to_owned())
    }
}

pub trait AsSymbol {
    fn as_symbol(&self) -> Symbol;
}

impl<T: AsRef<str>> AsSymbol for T {
    fn as_symbol(&self) -> Symbol {
        Symbol(self.as_ref().to_owned())
    }
}

impl AsSymbol for Symbol {
    fn as_symbol(&self) -> Symbol {
        self.clone()
    }
}

impl AsSymbol for &Symbol {
    fn as_symbol(&self) -> Symbol {
        (*self).clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct OrderId(pub u64);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LiveOrderRecord {
    pub id: OrderId,
    pub account_number: AccountNumber,
    pub time_in_force: TimeInForce,
    pub order_type: OrderType,
    pub size: u64,
    pub underlying_symbol: Symbol,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub price: Decimal,
    pub price_effect: PriceEffect,
    pub status: OrderStatus,
    pub cancellable: bool,
    pub editable: bool,
    pub edited: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LiveOrderLeg {
    pub instrument_type: InstrumentType,
    pub symbol: Symbol,
    pub quantity: u64,
    pub remaining_quantity: u64,
    pub action: Action,
    pub fills: Vec<String>,
}

#[derive(Builder, Serialize)]
#[serde(rename_all = "kebab-case")]
#[builder(setter(into))]
pub struct Order {
    time_in_force: TimeInForce,
    order_type: OrderType,

    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    price: Decimal,
    price_effect: PriceEffect,
    legs: Vec<OrderLeg>,
}

#[derive(Builder, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
#[builder(setter(into))]
pub struct OrderLeg {
    instrument_type: InstrumentType,
    symbol: Symbol,
    #[serde(with = "rust_decimal::serde::float")]
    quantity: Decimal,
    action: Action,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderPlacedResult {
    pub order: LiveOrderRecord,
    pub warnings: Vec<Warning>,
    pub buying_power_effect: BuyingPowerEffect,
    pub fee_calculation: FeeCalculation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DryRunResult {
    pub order: DryRunRecord,
    pub warnings: Vec<Warning>,
    pub buying_power_effect: BuyingPowerEffect,
    pub fee_calculation: FeeCalculation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DryRunRecord {
    pub account_number: AccountNumber,
    pub time_in_force: TimeInForce,
    pub order_type: OrderType,
    pub size: u64,
    pub underlying_symbol: Symbol,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub price: Decimal,
    pub price_effect: PriceEffect,
    pub status: OrderStatus,
    pub cancellable: bool,
    pub editable: bool,
    pub edited: bool,
    pub legs: Vec<OrderLeg>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuyingPowerEffect {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_in_margin_requirement: Decimal,
    pub change_in_margin_requirement_effect: PriceEffect,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_in_buying_power: Decimal,
    pub change_in_buying_power_effect: PriceEffect,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub current_buying_power: Decimal,
    pub current_buying_power_effect: PriceEffect,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub impact: Decimal,
    pub effect: PriceEffect,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeeCalculation {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub total_fees: Decimal,
    pub total_fees_effect: PriceEffect,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Warning {}
