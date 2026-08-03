//! What the venue currently allows through the API, as opposed to what an
//! account is entitled to.
//!
//! These are two different things and this module exists to keep them apart.
//! An account's [`crate::prelude::TradingStatus`] says what *it* may do;
//! nothing there tells a caller that tastytrade has switched a capability off
//! for every API client at once, which is what happened to cryptocurrency
//! trading on 2026-06-29.
//!
//! Everything temporary lives here, in constants, so restoring a capability is
//! a one-line change rather than an audit of the order paths. That is the whole
//! design: a suspension is a fact about a date, not a business rule to bake in.

use chrono::NaiveDate;

use crate::types::instrument::InstrumentType;
use crate::types::order::{Order, OrderLeg};

/// The day tastytrade disabled cryptocurrency trading through the API.
///
/// From the release notes: "Cryptocurrency trading has been disabled through
/// the API until further notice."
pub const CRYPTOCURRENCY_TRADING_SUSPENDED_ON: NaiveDate =
    match NaiveDate::from_ymd_opt(2026, 6, 29) {
        Some(date) => date,
        // Unreachable for a literal date, and a `const` panic is a compile error
        // rather than something a caller can hit.
        None => panic!("2026-06-29 is a real date"),
    };

/// Where that came from.
pub const CRYPTOCURRENCY_TRADING_SOURCE: &str = "https://developer.tastytrade.com/release-notes/";

/// Whether cryptocurrency **order routing** is available through the API.
///
/// The one switch. Flip it to `true` when the venue restores trading and every
/// guarded path opens at once; there is no second place to remember.
///
/// It says nothing about instrument discovery or market data, which are
/// unaffected — [`crate::TastyTrade::list_cryptocurrencies`] and the DXLink
/// feed keep working.
pub const CRYPTOCURRENCY_TRADING_ENABLED: bool = false;

/// Fails when an order's legs need a capability the venue has switched off.
///
/// A **local** refusal, so [`crate::TastyTradeError::Precondition`] and not
/// retryable: nothing was sent, and sending it again would fail the same way.
/// The alternative — let it go and rely on the venue's rejection — leaves the
/// crate advertising a capability the venue withdrew, which is the thing this
/// exists to stop.
///
/// A caller who finds the venue has restored trading before this crate has can
/// still reach the endpoint: [`crate::TastyTrade::post`] is public and
/// unguarded. That is the escape hatch, and it is deliberate — being wrong
/// about a suspension should cost an awkward call, not a blocked one.
pub(crate) fn ensure_legs_are_tradable(legs: &[OrderLeg]) -> crate::TastyResult<()> {
    if CRYPTOCURRENCY_TRADING_ENABLED {
        return Ok(());
    }

    if legs
        .iter()
        .any(|leg| *leg.instrument_type() == InstrumentType::Cryptocurrency)
    {
        return Err(crate::TastyTradeError::Precondition(format!(
            "tastytrade disabled cryptocurrency trading through the API on {}, so this \
             order cannot route; instrument data and market data are unaffected. \
             Source: {}",
            CRYPTOCURRENCY_TRADING_SUSPENDED_ON, CRYPTOCURRENCY_TRADING_SOURCE
        )));
    }

    Ok(())
}

/// The same check across every component of a container.
pub(crate) fn ensure_orders_are_tradable(orders: &[Order]) -> crate::TastyResult<()> {
    for order in orders {
        ensure_legs_are_tradable(order.legs())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::order::{
        Action, OrderBuilder, OrderLegBuilder, OrderType, PriceEffect, TimeInForce,
    };
    use rust_decimal::Decimal;

    fn leg(instrument_type: InstrumentType, symbol: &str) -> OrderLeg {
        OrderLegBuilder::default()
            .instrument_type(instrument_type)
            .symbol(symbol)
            .quantity(Decimal::ONE)
            .action(Action::BuyToOpen)
            .build()
            .expect("a valid leg")
    }

    fn order(legs: Vec<OrderLeg>) -> Order {
        OrderBuilder::default()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(Decimal::ONE)
            .price_effect(PriceEffect::Debit)
            .legs(legs)
            .build()
            .expect("a valid order")
    }

    #[test]
    fn a_cryptocurrency_leg_is_refused_while_the_suspension_stands() {
        let error = ensure_legs_are_tradable(&[leg(InstrumentType::Cryptocurrency, "BTC/USD")])
            .expect_err("crypto order routing is suspended");

        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable(), "nothing was sent");

        let rendered = format!("{error}");
        // The message has to say when and where from, or it reads as a bug in
        // this crate rather than a decision by the venue.
        assert!(rendered.contains("2026-06-29"), "{rendered}");
        assert!(rendered.contains("developer.tastytrade.com"), "{rendered}");
        assert!(
            rendered.contains("market data are unaffected"),
            "the message must not imply crypto data is gone too: {rendered}"
        );
    }

    #[test]
    fn every_other_instrument_type_is_untouched() {
        for instrument_type in [
            InstrumentType::Equity,
            InstrumentType::EquityOption,
            InstrumentType::Future,
            InstrumentType::FutureOption,
            InstrumentType::Bond,
            InstrumentType::Warrant,
        ] {
            assert!(
                ensure_legs_are_tradable(&[leg(instrument_type.clone(), "AAPL")]).is_ok(),
                "{instrument_type:?} must still be tradable"
            );
        }
    }

    /// A container is only as tradable as its least tradable component.
    #[test]
    fn one_cryptocurrency_component_refuses_the_whole_container() {
        let mixed = vec![
            order(vec![leg(InstrumentType::Equity, "AAPL")]),
            order(vec![leg(InstrumentType::Cryptocurrency, "BTC/USD")]),
        ];

        assert!(ensure_orders_are_tradable(&mixed).is_err());
        assert!(
            ensure_orders_are_tradable(&mixed[..1]).is_ok(),
            "the equity component on its own is fine"
        );
    }

    /// A multi-leg order with one crypto leg is still a crypto order.
    #[test]
    fn a_mixed_leg_order_is_refused() {
        assert!(
            ensure_legs_are_tradable(&[
                leg(InstrumentType::Equity, "AAPL"),
                leg(InstrumentType::Cryptocurrency, "BTC/USD"),
            ])
            .is_err()
        );
    }

    /// The tripwire for the day the switch flips.
    ///
    /// Asserted through the guard rather than on the constant directly, so it
    /// is a statement about behaviour: whichever way
    /// [`CRYPTOCURRENCY_TRADING_ENABLED`] is set, exactly one of these branches
    /// has to hold, and the failing one names what else needs updating.
    #[test]
    fn the_suspension_and_the_documentation_agree() {
        let refused =
            ensure_legs_are_tradable(&[leg(InstrumentType::Cryptocurrency, "BTC/USD")]).is_err();

        assert_eq!(
            refused, !CRYPTOCURRENCY_TRADING_ENABLED,
            "the guard and the switch disagree; if cryptocurrency API trading has \
             been re-enabled, update src/lib.rs, Doc/API_Coverage_Status.md, \
             Doc/Instruments_Implementation_Status.md and the crate docs to match"
        );
    }
}
