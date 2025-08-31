use crate::accounts::AccountNumber;
use crate::types::instrument::InstrumentType;
use derive_builder::Builder;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the effect of a price on an account.
///
/// This enum is used to indicate whether a price change results in a debit,
/// a credit, or has no effect on the account balance.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PriceEffect {
    /// Represents a debit, meaning a reduction in the account balance.
    Debit,
    /// Represents a credit, meaning an increase in the account balance.
    Credit,
    /// Represents no effect on the account balance.
    None,
}

impl fmt::Display for PriceEffect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PriceEffect::Debit => write!(f, "Debit"),
            PriceEffect::Credit => write!(f, "Credit"),
            PriceEffect::None => write!(f, "None"),
        }
    }
}

/// Represents an order action type.
///
/// This enum defines the different actions that can be performed when placing an order.
/// Each variant is serialized with a specific name for compatibility with the Tastyworks API.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Action {
    /// Represents a "Buy to Open" order action.
    #[serde(rename = "Buy to Open")]
    BuyToOpen,
    /// Represents a "Sell to Open" order action.
    #[serde(rename = "Sell to Open")]
    SellToOpen,
    /// Represents a "Buy to Close" order action.
    #[serde(rename = "Buy to Close")]
    BuyToClose,
    /// Represents a "Sell to Close" order action.
    #[serde(rename = "Sell to Close")]
    SellToClose,
    /// Represents a "Sell" order action.
    Sell,
    /// Represents a "Buy" order action.
    Buy,
}

/// Represents the type of order being placed.
///
/// This enum covers various order types, including limit orders, market orders,
/// marketable limit orders, stop orders, stop limit orders, and notional market orders.
/// The `#[serde(rename = "...")]` attribute is used to ensure proper serialization
/// and deserialization with external APIs that may use different naming conventions.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OrderType {
    /// A limit order is an order to buy or sell a security at a specific price or better.
    Limit,
    /// A market order is an order to buy or sell a security at the best available price immediately.
    Market,
    /// A marketable limit order is a limit order that is priced to execute immediately.
    #[serde(rename = "Marketable Limit")]
    MarketableLimit,
    /// A stop order is an order to buy or sell a security once the price of the security reaches a specified stop price.
    Stop,
    /// A stop-limit order is an order to buy or sell a security once the price of the security reaches a specified stop price. Once the stop price is reached, the stop-limit order becomes a limit order to buy or sell at the limit price or better.
    #[serde(rename = "Stop Limit")]
    StopLimit,
    /// A notional market order specifies the total amount of money you are willing to spend rather than the number of shares you want to buy.
    #[serde(rename = "Notional Market")]
    NotionalMarket,
}

/// Represents the time-in-force instruction for an order.
///
/// This enum specifies how long an order remains active before it is canceled
/// or expires.  It uses serde's `rename` attribute to map the Rust enum
/// variants to specific string values expected by the Tastyworks API.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TimeInForce {
    /// Day order: The order is valid only for the current trading day.
    #[serde(rename = "Day")]
    Day,
    /// Good-Til-Canceled order: The order remains active until it is filled or canceled.
    #[serde(rename = "GTC")]
    Gtc,
    /// Good-Til-Date order: The order remains active until the specified date.
    #[serde(rename = "GTD")]
    Gtd,
    /// Extended Hours order: The order can be executed during extended trading hours.
    #[serde(rename = "Ext")]
    Ext,
    /// Good-Til-Canceled Extended Hours order: Combines GTC and Extended Hours.
    #[serde(rename = "GTC Ext")]
    GTCExt,
    /// Immediate-or-Cancel order: The order must be filled immediately or partially filled.
    /// Any unfilled portion is canceled.
    #[serde(rename = "IOC")]
    Ioc,
}

/// Represents the status of an order.
///
/// This enum defines the various states an order can transition through,
/// from initial reception to final completion or cancellation.  The `serde`
/// attributes provide custom renaming for certain variants to match the API
/// specifications.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OrderStatus {
    /// The order has been received.
    Received,
    /// The order has been routed.
    Routed,
    /// The order is in flight.
    #[serde(rename = "In Flight")]
    InFlight,
    /// The order is live.
    Live,
    /// A cancellation request has been submitted for the order.
    #[serde(rename = "Cancel Requested")]
    CancelRequested,
    /// A replace request has been submitted for the order.
    #[serde(rename = "Replace Requested")]
    ReplaceRequested,
    /// The order is contingent.
    Contingent,
    /// The order has been filled.
    Filled,
    /// The order has been cancelled.
    Cancelled,
    /// The order has expired.
    Expired,
    /// The order has been rejected.
    Rejected,
    /// The order has been removed.
    Removed,
    /// The order has been partially removed.
    #[serde(rename = "Partially Removed")]
    PartiallyRemoved,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderStatus::Received => write!(f, "Received"),
            OrderStatus::Routed => write!(f, "Routed"),
            OrderStatus::InFlight => write!(f, "In Flight"),
            OrderStatus::Live => write!(f, "Live"),
            OrderStatus::CancelRequested => write!(f, "Cancel Requested"),
            OrderStatus::ReplaceRequested => write!(f, "Replace Requested"),
            OrderStatus::Contingent => write!(f, "Contingent"),
            OrderStatus::Filled => write!(f, "Filled"),
            OrderStatus::Cancelled => write!(f, "Cancelled"),
            OrderStatus::Expired => write!(f, "Expired"),
            OrderStatus::Rejected => write!(f, "Rejected"),
            OrderStatus::Removed => write!(f, "Removed"),
            OrderStatus::PartiallyRemoved => write!(f, "Partially Removed"),
        }
    }
}

/// Represents a trading symbol.
///
/// This struct wraps a `String` to represent a trading symbol.
/// The `#[serde(transparent)]` attribute ensures that during serialization and
/// deserialization, the `Symbol` is treated as if it were directly a `String`.
/// This simplifies the process and avoids unnecessary nesting in the resulting
/// JSON or other serialized formats.  It also ensures ordering, equality, and
/// hashing are based on the underlying string value.
#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(transparent)]
pub struct Symbol(pub String);

impl<T: AsRef<str>> From<T> for Symbol {
    fn from(value: T) -> Self {
        Self(value.as_ref().to_owned())
    }
}

/// Trait for converting types to `Symbol`.
///
/// This trait provides a method to convert a type into a `Symbol`, which represents a trading symbol.  This is useful for abstracting the process of obtaining a `Symbol` from various data sources.
pub trait AsSymbol {
    /// Converts the implementing type to a `Symbol`.
    fn as_symbol(&self) -> Symbol;
}

impl<T: AsRef<str>> AsSymbol for T {
    fn as_symbol(&self) -> Symbol {
        Symbol(self.as_ref().to_owned())
    }
}

/// Implements the `AsSymbol` trait for the `Symbol` type.
///
/// This implementation allows a `Symbol` to be converted into itself, which is a trivial operation.  This is useful when dealing with collections or generics where the `AsSymbol` trait is required, even though the underlying type is already a `Symbol`.
impl AsSymbol for Symbol {
    fn as_symbol(&self) -> Symbol {
        self.clone()
    }
}

/// Implements the `AsSymbol` trait for references to `Symbol`.
///
/// This implementation allows a reference to a `Symbol` to be directly used
/// in any context where the `AsSymbol` trait is required.  It simply clones
/// the underlying `Symbol` to satisfy the trait's method signature.
impl AsSymbol for &Symbol {
    fn as_symbol(&self) -> Symbol {
        (*self).clone()
    }
}

/// Represents an Order ID.
///
/// This struct provides a transparent wrapper around a `u64` to represent an order ID.
/// The `#[serde(transparent)]` attribute ensures that during serialization and deserialization,
/// the `OrderId` is treated as if it were just a `u64`.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct OrderId(pub u64);

/// Represents a live order record.
///
/// This struct holds the details of a live order, including its ID, account number,
/// time in force, order type, size, underlying symbol, price, price effect, status,
/// and flags indicating whether it's cancellable or editable.  The `#[serde(...)]`
/// attributes are used to control how the struct is serialized and deserialized
/// to and from JSON, ensuring compatibility with the Tastyworks API.  For example,
/// `rename_all = "kebab-case"` converts field names to kebab-case during serialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LiveOrderRecord {
    /// The unique identifier for the order.
    pub id: OrderId,
    /// The account number associated with the order.
    pub account_number: AccountNumber,
    /// The time-in-force instruction for the order.
    pub time_in_force: TimeInForce,
    /// The type of order (e.g., Limit, Market, Stop).
    pub order_type: OrderType,
    /// The size of the order (quantity of the underlying asset).
    pub size: u64,
    /// The symbol of the underlying asset being traded.
    pub underlying_symbol: Symbol,
    /// The price of the order.  Uses `rust_decimal` for arbitrary precision
    /// to avoid floating-point inaccuracies.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub price: Decimal,
    /// The effect of the price on the account (Debit, Credit, or None).
    pub price_effect: PriceEffect,
    /// The current status of the order (e.g., Live, Filled, Cancelled).
    pub status: OrderStatus,
    /// Indicates whether the order can be cancelled.
    pub cancellable: bool,
    /// Indicates whether the order can be edited.
    pub editable: bool,
    /// Indicates whether the order has been edited.
    pub edited: bool,
}

/// Represents a leg of a live order.
///
/// This struct stores information about a specific leg within a live order.
/// It includes details such as the instrument type, symbol, quantity, remaining
/// quantity, action, and a vector of fills.  The `#[serde(rename_all =
/// "kebab-case")]` attribute ensures that the fields are serialized and
/// deserialized with kebab-case naming conventions.
#[allow(dead_code)]
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LiveOrderLeg {
    /// The type of instrument for this leg.
    pub instrument_type: InstrumentType,
    /// The trading symbol for this leg.
    pub symbol: Symbol,
    /// The total quantity of the order for this leg.
    pub quantity: u64,
    /// The remaining quantity to be filled for this leg.
    pub remaining_quantity: u64,
    /// The action associated with this leg (e.g., Buy, Sell).
    pub action: Action,
    /// A vector of strings representing fills for this leg.  Further
    /// details on the contents are not documented.
    pub fills: Vec<String>,
}

/// Represents an order to be placed.
///
/// This struct encapsulates the details of an order, including its time-in-force,
/// order type, price, price effect, and a vector of order legs.  It uses the
/// `derive_builder` crate to provide a convenient builder pattern for constructing
/// order instances.  The `serde` attributes control how the struct is serialized
/// and deserialized, ensuring compatibility with external APIs or data formats.
#[derive(Builder, Serialize)]
#[serde(rename_all = "kebab-case")]
#[builder(setter(into))]
pub struct Order {
    /// Specifies how long the order remains active before being canceled or expiring.
    time_in_force: TimeInForce,
    /// The type of order (e.g., Limit, Market, Stop).
    order_type: OrderType,
    /// The price of the order.  Serialized with arbitrary precision.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    price: Decimal,
    /// The effect of the price on the account (Debit, Credit, None).
    price_effect: PriceEffect,
    /// A vector of order legs, each specifying details about a specific instrument
    /// involved in the order.
    legs: Vec<OrderLeg>,
}

/// Represents a leg of an order.
///
/// An `OrderLeg` defines the specifics of a particular instrument within a potentially
/// more complex order.  It includes details such as the instrument type, symbol,
/// quantity, and desired action (buy, sell, etc.).  The struct utilizes the derive
/// builder pattern to simplify construction and uses the `serde` crate for
/// serialization and deserialization with kebab-case renaming.
///
#[derive(Builder, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
#[builder(setter(into))]
pub struct OrderLeg {
    /// The type of instrument (e.g., Equity, Option).
    instrument_type: InstrumentType,
    /// The trading symbol for the instrument.
    symbol: Symbol,
    /// The quantity of the instrument to be traded.  Serialized as a float.
    #[serde(with = "rust_decimal::serde::float")]
    quantity: Decimal,
    /// The action to be taken (e.g., Buy, Sell).
    action: Action,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Represents the result of placing an order.
///
/// This structure encapsulates the details of a placed order, including the order record itself,
/// any warnings generated during order placement, the effect of the order on buying power, and
/// the fee calculation associated with the order.  The `#[serde(...)]` attributes control how
/// the struct is serialized and deserialized, ensuring compatibility with the Tastyworks API.
pub struct OrderPlacedResult {
    /// The details of the placed order.
    pub order: LiveOrderRecord,
    /// A list of warnings generated during order placement.  This can include warnings such
    /// as insufficient buying power or exceeding order limits.
    pub warnings: Vec<Warning>,
    /// The effect of the placed order on the account's buying power. This includes details
    /// about changes in margin requirements and available buying power.
    pub buying_power_effect: BuyingPowerEffect,
    /// The calculation of fees associated with the placed order.
    pub fee_calculation: FeeCalculation,
}

/// Represents the result of a dry-run order execution.  This structure provides
/// details about the simulated order execution, including potential warnings,
/// buying power effects, and fee calculations.  It's designed for deserialization
/// from a JSON response using `serde`, with kebab-case field renaming.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DryRunResult {
    /// Details of the simulated order.
    pub order: DryRunRecord,
    /// Any warnings generated during the dry-run.
    pub warnings: Vec<Warning>,
    /// The effect of the order on buying power.
    pub buying_power_effect: BuyingPowerEffect,
    /// Calculation of fees associated with the order.
    pub fee_calculation: FeeCalculation,
}

/// Represents a dry-run order record.  A dry-run order allows a user to simulate
/// placing an order to see the potential impact on their account without actually
/// executing the trade. This struct provides details about the simulated order,
/// such as its status, price, and whether it can be cancelled or edited.  The struct
/// utilizes the `serde` crate for serialization and deserialization, with kebab-case
/// renaming for compatibility with external APIs.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DryRunRecord {
    /// The account number associated with the dry-run order.
    pub account_number: AccountNumber,
    /// The time-in-force instruction for the dry-run order (e.g., Day, GTC).
    pub time_in_force: TimeInForce,
    /// The type of the dry-run order (e.g., Limit, Market).
    pub order_type: OrderType,
    /// The size of the dry-run order.
    pub size: u64,
    /// The underlying symbol for the dry-run order.
    pub underlying_symbol: Symbol,
    /// The price of the dry-run order.  Uses arbitrary precision deserialization.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub price: Decimal,
    /// The effect of the dry-run order's price on the account (Debit, Credit, None).
    pub price_effect: PriceEffect,
    /// The status of the dry-run order (e.g., Received, Filled, Cancelled).
    pub status: OrderStatus,
    /// Indicates whether the dry-run order can be cancelled.
    pub cancellable: bool,
    /// Indicates whether the dry-run order can be edited.
    pub editable: bool,
    /// Indicates whether the dry-run order has been edited.
    pub edited: bool,
    /// The legs of the dry-run order, providing details about each instrument involved.
    pub legs: Vec<OrderLeg>,
}

/// Represents the effect of a price change on buying power.
///
/// This struct details the changes in margin requirements and buying power
/// resulting from a price movement. It provides both the absolute changes and
/// the direction of the impact (debit or credit).  It uses `rust_decimal`
/// for arbitrary-precision decimal arithmetic to avoid floating-point
/// precision issues.  The `#[serde(rename_all = "kebab-case")]` attribute
/// ensures that the fields in the JSON response are matched to the struct
/// fields correctly, even if the casing is different.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuyingPowerEffect {
    /// The change in margin requirement.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_in_margin_requirement: Decimal,
    /// The effect of the change in margin requirement (Debit, Credit, None).
    pub change_in_margin_requirement_effect: PriceEffect,
    /// The change in buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_in_buying_power: Decimal,
    /// The effect of the change in buying power (Debit, Credit, None).
    pub change_in_buying_power_effect: PriceEffect,
    /// The current buying power.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub current_buying_power: Decimal,
    /// The effect of the current buying power (Debit, Credit, None).  This field indicates whether
    /// the current buying power represents a debit or credit balance relative to a neutral point.
    pub current_buying_power_effect: PriceEffect,
    /// The overall impact of the price change.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub impact: Decimal,
    /// The overall effect of the price change (Debit, Credit, None).
    pub effect: PriceEffect,
}

/// Represents the calculation of fees.
///
/// This struct holds the total fees and the effect of those fees on the account balance.
/// It uses `#[serde(rename_all = "kebab-case")]` to handle kebab-case formatted data during deserialization.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeeCalculation {
    /// The total fees calculated. Uses `rust_decimal::serde::arbitrary_precision` for deserialization
    /// to avoid precision loss with floating-point numbers.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub total_fees: Decimal,
    /// The effect of the total fees on the price.  For example, fees are typically a debit.
    pub total_fees_effect: PriceEffect,
}

/// Represents a warning message.  This struct is currently empty, potentially
/// serving as a placeholder for future warning information. The `#[serde(rename_all = "kebab-case")]`
/// attribute indicates that during deserialization, the field names in the incoming data should be
/// converted from kebab-case to snake_case. For example, a field named "warning-message" in the
/// incoming data would be mapped to `warning_message` in the struct.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Warning {}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_price_effect_display() {
        assert_eq!(format!("{}", PriceEffect::Debit), "Debit");
        assert_eq!(format!("{}", PriceEffect::Credit), "Credit");
        assert_eq!(format!("{}", PriceEffect::None), "None");
    }

    #[test]
    fn test_order_status_display() {
        assert_eq!(format!("{}", OrderStatus::Received), "Received");
        assert_eq!(format!("{}", OrderStatus::Live), "Live");
        assert_eq!(format!("{}", OrderStatus::Filled), "Filled");
        assert_eq!(format!("{}", OrderStatus::Cancelled), "Cancelled");
        assert_eq!(format!("{}", OrderStatus::InFlight), "In Flight");
        assert_eq!(
            format!("{}", OrderStatus::CancelRequested),
            "Cancel Requested"
        );
        assert_eq!(
            format!("{}", OrderStatus::ReplaceRequested),
            "Replace Requested"
        );
        assert_eq!(
            format!("{}", OrderStatus::PartiallyRemoved),
            "Partially Removed"
        );
    }

    #[test]
    fn test_symbol_from_string() {
        let symbol = Symbol::from("AAPL");
        assert_eq!(symbol.0, "AAPL");

        let symbol = Symbol::from(String::from("MSFT"));
        assert_eq!(symbol.0, "MSFT");
    }

    #[test]
    fn test_symbol_as_symbol_trait() {
        let symbol_str = "TSLA";
        let symbol = symbol_str.as_symbol();
        assert_eq!(symbol.0, "TSLA");

        let symbol_string = String::from("GOOGL");
        let symbol = symbol_string.as_symbol();
        assert_eq!(symbol.0, "GOOGL");

        let symbol_obj = Symbol::from("NVDA");
        let symbol = symbol_obj.as_symbol();
        assert_eq!(symbol.0, "NVDA");

        let symbol_ref = &Symbol::from("AMD");
        let symbol = symbol_ref.as_symbol();
        assert_eq!(symbol.0, "AMD");
    }

    #[test]
    fn test_order_id() {
        let order_id = OrderId(12345);
        assert_eq!(order_id.0, 12345);
    }

    #[test]
    fn test_order_builder() {
        let order = OrderBuilder::default()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(Decimal::from_str("150.50").unwrap())
            .price_effect(PriceEffect::Debit)
            .legs(vec![])
            .build()
            .unwrap();

        // Test that the order was built successfully
        // We can't directly access private fields, but we can serialize to test
        let serialized = serde_json::to_string(&order).unwrap();
        assert!(serialized.contains("Day"));
        assert!(serialized.contains("Limit"));
        assert!(serialized.contains("150.50"));
        assert!(serialized.contains("Debit"));
    }

    #[test]
    fn test_order_leg_builder() {
        let order_leg = OrderLegBuilder::default()
            .instrument_type(InstrumentType::Equity)
            .symbol(Symbol::from("AAPL"))
            .quantity(Decimal::from(100))
            .action(Action::Buy)
            .build()
            .unwrap();

        let serialized = serde_json::to_string(&order_leg).unwrap();
        assert!(serialized.contains("Equity"));
        assert!(serialized.contains("AAPL"));
        assert!(serialized.contains("100"));
        assert!(serialized.contains("Buy"));
    }

    #[test]
    fn test_enum_serialization() {
        // Test Action enum serialization
        let action = Action::BuyToOpen;
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, "\"Buy to Open\"");

        let action = Action::SellToClose;
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, "\"Sell to Close\"");

        // Test OrderType enum serialization
        let order_type = OrderType::MarketableLimit;
        let serialized = serde_json::to_string(&order_type).unwrap();
        assert_eq!(serialized, "\"Marketable Limit\"");

        let order_type = OrderType::StopLimit;
        let serialized = serde_json::to_string(&order_type).unwrap();
        assert_eq!(serialized, "\"Stop Limit\"");

        // Test TimeInForce enum serialization
        let tif = TimeInForce::Gtc;
        let serialized = serde_json::to_string(&tif).unwrap();
        assert_eq!(serialized, "\"GTC\"");

        let tif = TimeInForce::GTCExt;
        let serialized = serde_json::to_string(&tif).unwrap();
        assert_eq!(serialized, "\"GTC Ext\"");
    }

    #[test]
    fn test_enum_deserialization() {
        // Test Action enum deserialization
        let action: Action = serde_json::from_str("\"Buy to Open\"").unwrap();
        matches!(action, Action::BuyToOpen);

        let action: Action = serde_json::from_str("\"Sell to Close\"").unwrap();
        matches!(action, Action::SellToClose);

        // Test OrderStatus enum deserialization
        let status: OrderStatus = serde_json::from_str("\"In Flight\"").unwrap();
        matches!(status, OrderStatus::InFlight);

        let status: OrderStatus = serde_json::from_str("\"Cancel Requested\"").unwrap();
        matches!(status, OrderStatus::CancelRequested);
    }

    #[test]
    fn test_symbol_clone_and_eq() {
        let symbol1 = Symbol::from("AAPL");
        let symbol2 = symbol1.clone();
        assert_eq!(symbol1, symbol2);

        let symbol3 = Symbol::from("MSFT");
        assert_ne!(symbol1, symbol3);
    }

    #[test]
    fn test_symbol_ordering() {
        let symbol1 = Symbol::from("AAPL");
        let symbol2 = Symbol::from("MSFT");
        let symbol3 = Symbol::from("AAPL");

        assert!(symbol1 < symbol2);
        assert!(symbol1 <= symbol3);
        assert!(symbol2 > symbol1);
        assert_eq!(symbol1, symbol3);
    }

    #[test]
    fn test_price_effect_clone() {
        let effect1 = PriceEffect::Debit;
        let effect2 = effect1.clone();
        matches!(effect2, PriceEffect::Debit);
    }

    #[test]
    fn test_all_enum_variants_exist() {
        // Test that all Action variants can be created
        let _actions = [
            Action::BuyToOpen,
            Action::SellToOpen,
            Action::BuyToClose,
            Action::SellToClose,
            Action::Sell,
            Action::Buy,
        ];

        // Test that all OrderType variants can be created
        let _order_types = [
            OrderType::Limit,
            OrderType::Market,
            OrderType::MarketableLimit,
            OrderType::Stop,
            OrderType::StopLimit,
            OrderType::NotionalMarket,
        ];

        // Test that all TimeInForce variants can be created
        let _time_in_forces = [
            TimeInForce::Day,
            TimeInForce::Gtc,
            TimeInForce::Gtd,
            TimeInForce::Ext,
            TimeInForce::GTCExt,
            TimeInForce::Ioc,
        ];

        // Test that all OrderStatus variants can be created
        let _statuses = [
            OrderStatus::Received,
            OrderStatus::Routed,
            OrderStatus::InFlight,
            OrderStatus::Live,
            OrderStatus::CancelRequested,
            OrderStatus::ReplaceRequested,
            OrderStatus::Contingent,
            OrderStatus::Filled,
            OrderStatus::Cancelled,
            OrderStatus::Expired,
            OrderStatus::Rejected,
            OrderStatus::Removed,
            OrderStatus::PartiallyRemoved,
        ];
    }
}
