//! Contingent orders: OCO, OTOCO, PAIRS and the rest.
//!
//! A complex order is a container of component orders whose fates are linked —
//! "take profit or stop out, whichever comes first" is one object at the venue,
//! not two orders and a race. Cancelling the container requests cancellation of
//! every component that is not already terminal.
//!
//! The response shape is not an [`crate::prelude::Order`], so it has its own
//! types. Component identifiers are **strings** here, not the `u64` a plain
//! order carries.

use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::instrument::InstrumentType;
use crate::types::order::{Order, OrderStatus, OrderType, PriceEffect, Symbol, TimeInForce};
use crate::types::wire::wire_enum;

wire_enum! {
    /// How the components of a complex order are linked.
    ///
    /// The five strategies the venue enumerates.
    ComplexOrderType {
        Blast => "BLAST",
        Oco => "OCO",
        Oto => "OTO",
        Otoco => "OTOCO",
        Pairs => "PAIRS",
    }
}

/// How a PAIRS threshold is compared against the ratio price.
///
/// A closed enum with no `Unknown` arm: this is a **request** value and the
/// venue enumerates exactly two. Tolerance matters when reading, where an
/// unrecognised value would otherwise be lost; on write it would only let a
/// caller send something the venue rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RatioPriceComparator {
    /// Greater than or equal to.
    #[serde(rename = "gte")]
    GreaterOrEqual,
    /// Less than or equal to.
    #[serde(rename = "lte")]
    LessOrEqual,
}

impl RatioPriceComparator {
    /// The text the venue uses.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::GreaterOrEqual => "gte",
            Self::LessOrEqual => "lte",
        }
    }
}

/// A complex order's identifier.
///
/// A `String`, not a `u64`: the venue's schema types complex-order identifiers
/// as strings while plain orders get integers, and assuming otherwise would
/// fail to decode the first response.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ComplexOrderId(pub String);

impl<T: AsRef<str>> From<T> for ComplexOrderId {
    fn from(value: T) -> Self {
        Self(value.as_ref().to_owned())
    }
}

/// A contingent order and its components.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ComplexOrder {
    /// The venue's identifier.
    #[serde(default)]
    pub id: Option<ComplexOrderId>,
    /// Which account it belongs to. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
    /// How the components are linked.
    #[serde(rename = "type", default)]
    pub complex_order_type: Option<ComplexOrderType>,
    /// How the PAIRS threshold is compared.
    #[serde(default)]
    pub ratio_price_comparator: Option<RatioPriceComparator>,
    /// The PAIRS threshold.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub ratio_price_threshold: Option<Decimal>,
    /// Whether the threshold is expressed against notional value.
    #[serde(default)]
    pub ratio_price_is_threshold_based_on_notional: Option<bool>,
    /// When the whole thing became terminal, if it has.
    #[serde(default)]
    pub terminal_at: Option<String>,
    /// The current component orders.
    #[serde(default)]
    pub orders: Vec<ComplexOrderComponent>,
    /// Replaced, unfilled and terminal orders, as the venue describes them.
    #[serde(default)]
    pub related_orders: Vec<RelatedOrder>,
}

impl ComplexOrder {
    /// Whether any component can still change.
    ///
    /// A component whose status this crate does not recognise counts as **not**
    /// terminal: an unknown status says nothing about whether the order is
    /// done, and answering "finished" would stop a caller watching something
    /// still working.
    pub fn has_working_components(&self) -> bool {
        self.orders
            .iter()
            .any(|order| !order.status.as_ref().is_some_and(OrderStatus::is_terminal))
    }
}

/// One component order of a [`ComplexOrder`].
///
/// Modelled separately from [`crate::prelude::LiveOrderRecord`] because the
/// identifiers are strings and almost every field is optional here — the venue
/// sends what applies to the component's stage.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ComplexOrderComponent {
    /// The component's identifier.
    #[serde(default)]
    pub id: Option<String>,
    /// Which account it belongs to. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
    /// Which container it belongs to.
    #[serde(default)]
    pub complex_order_id: Option<String>,
    /// The venue's tag for this component's role.
    #[serde(default)]
    pub complex_order_tag: Option<String>,
    /// Its current status.
    #[serde(default)]
    pub status: Option<OrderStatus>,
    /// Whether it is waiting on a trigger.
    #[serde(default)]
    pub contingent_status: Option<String>,
    /// What kind of order it is.
    #[serde(default)]
    pub order_type: Option<OrderType>,
    /// How long it rests.
    #[serde(default)]
    pub time_in_force: Option<TimeInForce>,
    /// When a good-til-date component expires.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub gtc_date: Option<NaiveDate>,
    /// Its size.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub size: Option<Decimal>,
    /// The underlying.
    #[serde(default)]
    pub underlying_symbol: Option<Symbol>,
    /// What kind of instrument the underlying is.
    #[serde(default)]
    pub underlying_instrument_type: Option<InstrumentType>,
    /// Its price, for order types that have one.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price: Option<Decimal>,
    /// Whether the price is a debit or a credit.
    #[serde(default)]
    pub price_effect: Option<PriceEffect>,
    /// Its notional value.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub value: Option<Decimal>,
    /// Whether the value is a debit or a credit.
    #[serde(default)]
    pub value_effect: Option<PriceEffect>,
    /// The stop trigger, for stop and stop-limit components.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub stop_trigger: Option<Decimal>,
    /// Whether it can be cancelled.
    #[serde(default)]
    pub cancellable: Option<bool>,
    /// Whether it can be edited.
    #[serde(default)]
    pub editable: Option<bool>,
    /// Whether it has been edited.
    #[serde(default)]
    pub edited: Option<bool>,
    /// How much of it was cancelled.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub cancelled_size: Option<Decimal>,
    /// Why the venue rejected it, when it did.
    ///
    /// Venue prose written for a person; it belongs in front of somebody
    /// rather than in `tracing`.
    #[serde(default)]
    pub reject_reason: Option<String>,
    /// The order this one replaces.
    #[serde(default)]
    pub replaces_order_id: Option<String>,
    /// The order replacing this one.
    #[serde(default)]
    pub replacing_order_id: Option<String>,
    /// How many legs it has.
    #[serde(default)]
    pub leg_count: Option<i64>,
    /// Its legs, as the venue echoes them.
    #[serde(default)]
    pub legs: Vec<serde_json::Value>,
    /// When it went live, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub live_at: Option<DateTime<FixedOffset>>,
    /// When it was received, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub received_at: Option<DateTime<FixedOffset>>,
    /// When it was cancelled, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub cancelled_at: Option<DateTime<FixedOffset>>,
    /// When it became terminal.
    #[serde(default)]
    pub terminal_at: Option<String>,
}

/// A replaced, unfilled or terminal order attached to a complex order.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct RelatedOrder {
    /// Its identifier.
    #[serde(default)]
    pub id: Option<String>,
    /// Which container it belongs to.
    #[serde(default)]
    pub complex_order_id: Option<String>,
    /// The venue's tag for its role.
    #[serde(default)]
    pub complex_order_tag: Option<String>,
    /// The order it replaces.
    #[serde(default)]
    pub replaces_order_id: Option<String>,
    /// The order replacing it.
    #[serde(default)]
    pub replacing_order_id: Option<String>,
    /// Its status.
    #[serde(default)]
    pub status: Option<OrderStatus>,
}

/// A contingent order to place.
///
/// Built through [`ComplexOrderRequest::new`] and validated before anything is
/// sent. The component orders are ordinary [`crate::prelude::Order`]s, so they
/// inherit the leg rules the order builder already enforces — a positive
/// quantity and a non-empty symbol.
#[derive(DebugPretty, DisplaySimple, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ComplexOrderRequest {
    /// How the components are linked.
    #[serde(rename = "type")]
    pub complex_order_type: ComplexOrderType,
    /// The component orders.
    pub orders: Vec<Order>,
    /// How the PAIRS threshold is compared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio_price_comparator: Option<RatioPriceComparator>,
    /// The PAIRS threshold.
    #[serde(
        skip_serializing_if = "Option::is_none",
        with = "crate::types::wire::decimal_option"
    )]
    pub ratio_price_threshold: Option<Decimal>,
    /// Whether the threshold is expressed against notional value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio_price_is_threshold_based_on_notional: Option<bool>,
}

/// The fewest components each strategy needs.
///
/// `OCO` is "one cancels other" and `OTOCO` is "one triggers a one-cancels-other
/// pair", so neither means anything with a single order. The rest take one.
const fn minimum_components(complex_order_type: &ComplexOrderType) -> usize {
    // Exhaustive with **no wildcard**, deliberately: a strategy the venue adds
    // later must break this build rather than inherit whichever default a `_`
    // arm happened to give it. The `Unknown` arm is named for the same reason —
    // it is a real case with a real answer, not a catch-all.
    match complex_order_type {
        ComplexOrderType::Oco | ComplexOrderType::Otoco => 2,
        ComplexOrderType::Blast | ComplexOrderType::Oto | ComplexOrderType::Pairs => 1,
        // A strategy this crate does not recognise: require at least one
        // component and let the venue apply its own rule.
        ComplexOrderType::Unknown(_) => 1,
    }
}

impl ComplexOrderRequest {
    /// A complex order of `complex_order_type` made of `orders`.
    pub fn new(complex_order_type: ComplexOrderType, orders: Vec<Order>) -> Self {
        Self {
            complex_order_type,
            orders,
            ratio_price_comparator: None,
            ratio_price_threshold: None,
            ratio_price_is_threshold_based_on_notional: None,
        }
    }

    /// Sets the PAIRS threshold and how it is compared.
    #[must_use]
    pub fn with_ratio_price(
        mut self,
        comparator: RatioPriceComparator,
        threshold: Decimal,
    ) -> Self {
        self.ratio_price_comparator = Some(comparator);
        self.ratio_price_threshold = Some(threshold);
        self
    }

    /// Whether the threshold is against notional value.
    #[must_use]
    pub fn with_threshold_based_on_notional(mut self, based_on_notional: bool) -> Self {
        self.ratio_price_is_threshold_based_on_notional = Some(based_on_notional);
        self
    }

    /// Fails when the request cannot be what the venue accepts.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable: nothing was sent, and this is a money path where a rejected
    /// container can leave some components placed.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        let minimum = minimum_components(&self.complex_order_type);
        if self.orders.len() < minimum {
            return Err(crate::TastyTradeError::Precondition(format!(
                "a {} complex order needs at least {minimum} component order(s), \
                 and this one has {}",
                self.complex_order_type.as_wire(),
                self.orders.len()
            )));
        }

        // The venue marks both required, and a PAIRS trade with no threshold is
        // a trade with no trigger.
        if self.complex_order_type == ComplexOrderType::Pairs
            && (self.ratio_price_comparator.is_none() || self.ratio_price_threshold.is_none())
        {
            return Err(crate::TastyTradeError::Precondition(
                "a PAIRS complex order needs a ratio price threshold and a comparator".to_string(),
            ));
        }

        Ok(())
    }
}

/// An edit to a PAIRS trade's threshold price.
///
/// The **only** thing `PATCH /complex-orders/{id}` changes — narrower than the
/// plain-order patch, which is why it is its own type rather than a generic
/// edit that would advertise fields this route ignores.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PairsThresholdEdit {
    /// How the threshold is compared.
    pub ratio_price_comparator: RatioPriceComparator,
    /// The new threshold.
    #[serde(with = "crate::types::wire::decimal")]
    pub ratio_price_threshold: Decimal,
}

impl PairsThresholdEdit {
    /// A new threshold and comparator.
    pub fn new(
        ratio_price_comparator: RatioPriceComparator,
        ratio_price_threshold: Decimal,
    ) -> Self {
        Self {
            ratio_price_comparator,
            ratio_price_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::order::{Action, OrderBuilder, OrderLegBuilder};

    fn component() -> Order {
        OrderBuilder::default()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(Decimal::ONE)
            .price_effect(PriceEffect::Debit)
            .legs(vec![
                OrderLegBuilder::default()
                    .instrument_type(InstrumentType::Equity)
                    .symbol("AAPL")
                    .quantity(Decimal::ONE)
                    .action(Action::BuyToOpen)
                    .build()
                    .expect("a valid leg"),
            ])
            .build()
            .expect("a valid order")
    }

    /// OCO means "one cancels other". With one component there is no other.
    #[test]
    fn a_one_sided_oco_is_refused_locally() {
        let error = ComplexOrderRequest::new(ComplexOrderType::Oco, vec![component()])
            .validate()
            .expect_err("OCO needs two components");

        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable());
        assert!(format!("{error}").contains("at least 2"), "{error}");
    }

    #[test]
    fn a_two_sided_oco_is_accepted() {
        assert!(
            ComplexOrderRequest::new(ComplexOrderType::Oco, vec![component(), component()])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn a_container_with_no_components_is_refused() {
        for kind in [
            ComplexOrderType::Blast,
            ComplexOrderType::Oto,
            ComplexOrderType::Oco,
            ComplexOrderType::Otoco,
            ComplexOrderType::Pairs,
        ] {
            assert!(
                ComplexOrderRequest::new(kind, vec![]).validate().is_err(),
                "an empty container must be refused"
            );
        }
    }

    /// A PAIRS trade with no threshold has no trigger, and the venue marks
    /// both fields required.
    #[test]
    fn a_pairs_trade_needs_its_threshold() {
        let error = ComplexOrderRequest::new(ComplexOrderType::Pairs, vec![component()])
            .validate()
            .expect_err("PAIRS needs a threshold");
        assert!(format!("{error}").contains("threshold"), "{error}");

        assert!(
            ComplexOrderRequest::new(ComplexOrderType::Pairs, vec![component()])
                .with_ratio_price(RatioPriceComparator::GreaterOrEqual, Decimal::ONE)
                .validate()
                .is_ok()
        );
    }

    /// A strategy this crate has not seen still goes out, with the venue's own
    /// spelling, and is held to the weakest rule rather than rejected.
    #[test]
    fn an_unknown_strategy_is_still_expressible() {
        let request = ComplexOrderRequest::new(
            ComplexOrderType::from("SOMETHING NEW".to_string()),
            vec![component()],
        );

        assert!(request.validate().is_ok());
        let body = serde_json::to_value(&request).expect("serialises");
        assert_eq!(body["type"], "SOMETHING NEW");
    }

    /// The threshold fields are omitted when unset rather than sent as null: a
    /// non-PAIRS container has no threshold, and `null` is a different request
    /// from no field.
    #[test]
    fn the_ratio_fields_are_omitted_when_unset() {
        let body = serde_json::to_value(ComplexOrderRequest::new(
            ComplexOrderType::Oco,
            vec![component(), component()],
        ))
        .expect("serialises");

        assert!(body.get("ratio-price-comparator").is_none(), "{body}");
        assert!(body.get("ratio-price-threshold").is_none(), "{body}");
        assert_eq!(body["type"], "OCO");
        assert_eq!(body["orders"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn a_pairs_threshold_edit_serialises_the_venues_spelling() {
        let body = serde_json::to_value(PairsThresholdEdit::new(
            RatioPriceComparator::LessOrEqual,
            Decimal::new(125, 2),
        ))
        .expect("serialises");

        assert_eq!(body["ratio-price-comparator"], "lte");
        assert_eq!(body["ratio-price-threshold"], serde_json::json!(1.25));
    }

    /// A component whose status this crate has not seen is **not** assumed
    /// finished: that would stop a caller watching something still working.
    #[test]
    fn an_unrecognised_component_status_counts_as_working() {
        let order: ComplexOrder = serde_json::from_str(
            r#"{"id": "abc", "type": "OCO",
                "orders": [{"id": "1", "status": "Filled"},
                           {"id": "2", "status": "Something New"}]}"#,
        )
        .expect("the container must decode");

        assert_eq!(order.complex_order_type, Some(ComplexOrderType::Oco));
        assert!(order.has_working_components());

        let done: ComplexOrder =
            serde_json::from_str(r#"{"orders": [{"status": "Filled"}, {"status": "Cancelled"}]}"#)
                .expect("the container must decode");
        assert!(!done.has_working_components());
    }
}
