//! Wire-format helpers for values the venue is inconsistent about.
//!
//! tastytrade sends the same quantity as a JSON number on one endpoint and a
//! quoted decimal on another, and it can send a fractional quantity anywhere a
//! whole one was expected — cryptocurrencies trade in fractions, and so do
//! some corporate-action-adjusted equity positions.
//!
//! Going through `f64` to smooth that over is what these helpers exist to
//! avoid: `0.1 + 0.2` is the reason money is not a float, and a quantity is
//! money's twin. `serde_json`'s `arbitrary_precision` feature is enabled
//! through `rust_decimal`, so a `Number` here still holds the digits the venue
//! actually sent and `to_string` returns them unchanged.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serializer};
use std::str::FromStr;

/// The two shapes a numeric field arrives in.
#[derive(Deserialize)]
#[serde(untagged)]
enum WireNumber {
    /// `"1.5"`, which is what most money-shaped fields use.
    Text(String),
    /// `1.5`, which is what most quantity-shaped fields use.
    Number(serde_json::Number),
}

impl WireNumber {
    fn into_decimal<E: serde::de::Error>(self) -> Result<Decimal, E> {
        let text = match self {
            WireNumber::Text(text) => text,
            // Lossless: with arbitrary_precision the Number keeps the original
            // digits, so this is the venue's own text rather than a rounded
            // rendering of a float.
            WireNumber::Number(number) => number.to_string(),
        };

        Decimal::from_str(text.trim()).map_err(|e| {
            E::custom(format!(
                "expected a decimal quantity, got something unparseable ({e})"
            ))
        })
    }
}

/// A quantity that must not lose precision, sent by the venue as either a
/// quoted decimal or a JSON number.
pub(crate) mod decimal {
    use super::*;

    /// Accepts both shapes and never routes through `f64`.
    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        WireNumber::deserialize(deserializer)?.into_decimal()
    }

    /// Emits a JSON number with every digit intact.
    ///
    /// Deliberately not `Decimal`'s own `Serialize`, which writes a quoted
    /// string. These fields go into order payloads, the previous
    /// implementation sent numbers, and this is not the place to find out
    /// whether the venue also accepts strings. `arbitrary_precision` keeps the
    /// digits that `serde::float` used to round off.
    pub(crate) fn serialize<S>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        rust_decimal::serde::arbitrary_precision::serialize(value, serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, serde::Serialize)]
    struct Holder {
        #[serde(with = "decimal")]
        quantity: Decimal,
    }

    #[test]
    fn both_wire_shapes_produce_the_same_value() {
        let from_number: Holder =
            serde_json::from_str(r#"{"quantity":1.5}"#).expect("a JSON number parses");
        let from_text: Holder =
            serde_json::from_str(r#"{"quantity":"1.5"}"#).expect("a quoted decimal parses");

        assert_eq!(from_number.quantity, Decimal::from_str("1.5").unwrap());
        assert_eq!(from_number.quantity, from_text.quantity);
    }

    /// The reason none of this goes through f64. 0.1 has no exact binary
    /// representation, so a float round-trip changes the value.
    #[test]
    fn a_fractional_quantity_survives_intact() {
        let holder: Holder = serde_json::from_str(r#"{"quantity":0.12345678901234567890}"#)
            .expect("a long fraction parses");

        assert_eq!(
            holder.quantity.to_string(),
            "0.12345678901234567890",
            "digits were lost on the way in"
        );
    }

    #[test]
    fn whole_quantities_still_work_from_either_shape() {
        for body in [r#"{"quantity":7}"#, r#"{"quantity":"7"}"#] {
            let holder: Holder = serde_json::from_str(body).expect("whole numbers parse");
            assert_eq!(holder.quantity, Decimal::from(7));
        }
    }

    #[test]
    fn a_value_that_is_not_a_number_is_an_error_not_a_zero() {
        let error = serde_json::from_str::<Holder>(r#"{"quantity":"not a number"}"#)
            .expect_err("garbage must not silently become zero");

        assert!(
            error.to_string().contains("decimal quantity"),
            "the error should say what was expected: {error}"
        );
    }

    /// Order payloads carry these fields, and the previous implementation sent
    /// numbers. Changing that shape is not something to discover in production.
    #[test]
    fn serialization_keeps_the_json_number_shape() {
        let holder = Holder {
            quantity: Decimal::from_str("2.5").unwrap(),
        };

        assert_eq!(
            serde_json::to_string(&holder).expect("Holder serializes"),
            r#"{"quantity":2.5}"#
        );
    }

    /// The round trip that `serde::float` could not do.
    #[test]
    fn a_long_fraction_round_trips_through_serialization() {
        let original = Decimal::from_str("0.12345678901234567890").unwrap();
        let json = serde_json::to_string(&Holder { quantity: original }).expect("serializes");
        let back: Holder = serde_json::from_str(&json).expect("parses back");

        assert_eq!(
            back.quantity, original,
            "a digit was lost in the round trip"
        );
    }
}
