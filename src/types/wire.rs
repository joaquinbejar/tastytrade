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
                "expected a decimal value, got something unparseable ({e})"
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

/// The same, for a field the venue may omit entirely.
pub(crate) mod decimal_option {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<WireNumber>::deserialize(deserializer)? {
            Some(raw) => raw.into_decimal().map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn serialize<S>(value: &Option<Decimal>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => super::decimal::serialize(value, serializer),
            None => serializer.serialize_none(),
        }
    }
}

/// A field the venue is inconsistent about the *shape* of, not just the
/// encoding: a JSON string on one path and a JSON number on another, with no
/// captured frame to settle which.
///
/// Both are kept as text, unchanged. The alternative to a helper like this is
/// picking one shape and having the whole surrounding object fail to decode
/// whenever the other arrives — which on the streaming path turns a routine
/// notification into an unreadable frame.
///
/// Use it only where the sources genuinely disagree. A field with one
/// documented shape should have that shape's type.
pub(crate) mod loose_string_option {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Option::<WireNumber>::deserialize(deserializer)? {
            Some(WireNumber::Text(text)) => Some(text),
            // With `arbitrary_precision` the Number keeps the venue's own
            // digits, so this is a copy rather than a rendering.
            Some(WireNumber::Number(number)) => Some(number.to_string()),
            None => None,
        })
    }

    pub(crate) fn serialize<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_str(value),
            None => serializer.serialize_none(),
        }
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
            error.to_string().contains("decimal value"),
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

/// An instant the venue sends as RFC 3339, e.g. `2025-09-19T13:30:00.000+00:00`.
///
/// Kept as `DateTime<FixedOffset>` rather than converted to UTC: the offset is
/// information the venue chose to send, and a caller who wants UTC is one
/// `with_timezone` away, while a caller who wanted the original offset cannot
/// recover it once it is gone.
pub(crate) mod datetime {
    use super::*;
    use chrono::{DateTime, FixedOffset};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(text.trim())
            .map_err(|e| serde::de::Error::custom(format!("expected an RFC 3339 timestamp ({e})")))
    }

    pub(crate) fn serialize<S>(
        value: &DateTime<FixedOffset>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_rfc3339())
    }
}

/// The same, for an instant the venue may omit.
pub(crate) mod datetime_option {
    use super::*;
    use chrono::{DateTime, FixedOffset};

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<DateTime<FixedOffset>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<String>::deserialize(deserializer)? {
            Some(text) => DateTime::parse_from_rfc3339(text.trim())
                .map(Some)
                .map_err(|e| {
                    serde::de::Error::custom(format!("expected an RFC 3339 timestamp ({e})"))
                }),
            None => Ok(None),
        }
    }

    pub(crate) fn serialize<S>(
        value: &Option<DateTime<FixedOffset>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_str(&value.to_rfc3339()),
            None => serializer.serialize_none(),
        }
    }
}

/// A calendar date with no time and no zone, e.g. `2025-09-19`.
///
/// Deliberately `NaiveDate`: an expiration date is a day on an exchange
/// calendar, not an instant. Attaching a timezone would invent information the
/// venue did not send and make the value wrong for half the world.
pub(crate) mod date {
    use super::*;
    use chrono::NaiveDate;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
            .map_err(|e| serde::de::Error::custom(format!("expected a YYYY-MM-DD date ({e})")))
    }

    pub(crate) fn serialize<S>(value: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.format("%Y-%m-%d").to_string())
    }
}

/// The same, for a date the venue may omit.
pub(crate) mod date_option {
    use super::*;
    use chrono::NaiveDate;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<String>::deserialize(deserializer)? {
            Some(text) => NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
                .map(Some)
                .map_err(|e| serde::de::Error::custom(format!("expected a YYYY-MM-DD date ({e})"))),
            None => Ok(None),
        }
    }

    pub(crate) fn serialize<S>(value: &Option<NaiveDate>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_str(&value.format("%Y-%m-%d").to_string()),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod time_tests {
    use super::*;
    use chrono::{DateTime, Datelike, FixedOffset, NaiveDate};
    use serde::Deserialize;

    #[derive(Debug, Deserialize, serde::Serialize)]
    struct Instant {
        #[serde(with = "datetime")]
        at: DateTime<FixedOffset>,
    }

    #[derive(Debug, Deserialize, serde::Serialize)]
    struct Day {
        #[serde(with = "date")]
        on: NaiveDate,
    }

    /// The shape the venue actually sends, taken from a captured futures
    /// option chain.
    #[test]
    fn an_rfc_3339_instant_keeps_its_offset() {
        let parsed: Instant =
            serde_json::from_str(r#"{"at":"2025-09-19T13:30:00.000+00:00"}"#).expect("parses");

        assert_eq!(parsed.at.date_naive().day(), 19);
        assert_eq!(
            parsed.at,
            DateTime::parse_from_rfc3339("2025-09-19T13:30:00+00:00").unwrap()
        );
    }

    /// An expiration is a day on an exchange calendar, not an instant.
    /// Attaching a timezone would invent information the venue did not send.
    #[test]
    fn a_calendar_date_stays_naive() {
        let parsed: Day = serde_json::from_str(r#"{"on":"2025-09-19"}"#).expect("parses");
        assert_eq!(parsed.on, NaiveDate::from_ymd_opt(2025, 9, 19).unwrap());

        assert_eq!(
            serde_json::to_string(&parsed).expect("serializes"),
            r#"{"on":"2025-09-19"}"#,
            "the wire shape must round trip unchanged"
        );
    }

    /// Dates and instants are different wire formats and must not be
    /// interchangeable, or a date-only field would silently accept a timestamp
    /// and lose its time.
    #[test]
    fn the_two_formats_do_not_accept_each_other() {
        serde_json::from_str::<Day>(r#"{"on":"2025-09-19T13:30:00.000+00:00"}"#)
            .expect_err("a timestamp is not a calendar date");
        serde_json::from_str::<Instant>(r#"{"at":"2025-09-19"}"#)
            .expect_err("a calendar date is not an instant");
    }

    #[test]
    fn an_unparseable_value_says_what_was_expected() {
        let error = serde_json::from_str::<Day>(r#"{"on":"19/09/2025"}"#)
            .expect_err("a non-ISO date must not parse");
        assert!(error.to_string().contains("YYYY-MM-DD"), "{error}");
    }
}

/// Declares a closed set the venue owns, with an escape hatch it cannot break.
///
/// The broker adds values without notice, and this crate must not fail a whole
/// listing over one it has not seen — `Items<T>` would drop the instrument and
/// the caller would get a short list instead of an error. Every generated enum
/// therefore has an `Unknown(String)` arm that round-trips the original text
/// unchanged, so an unrecognised value is visible, matchable and preserved on
/// the way back out.
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $($variant:ident => $wire:literal),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                #[doc = concat!("The venue's `", $wire, "`.")]
                $variant,
            )+
            /// A value this crate has not seen, kept verbatim.
            ///
            /// Matching on it is how you find out the broker added something.
            Unknown(String),
        }

        impl $name {
            /// The text the venue uses for this value.
            pub fn as_wire(&self) -> &str {
                match self {
                    $( $name::$variant => $wire, )+
                    $name::Unknown(text) => text,
                }
            }

            /// Whether this is a value the crate recognises.
            pub fn is_known(&self) -> bool {
                !matches!(self, $name::Unknown(_))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_wire())
            }
        }

        impl From<String> for $name {
            fn from(text: String) -> Self {
                // Trimmed for matching, like every other helper here, so a
                // stray space does not turn a known value into an unknown one.
                // A known value is then canonical: "PM " serialises back as
                // "PM". An unknown keeps the venue's text byte for byte,
                // since that is the only record of what arrived.
                match text.trim() {
                    $( $wire => $name::$variant, )+
                    _ => $name::Unknown(text),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(Self::from(String::deserialize(deserializer)?))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_wire())
            }
        }
    };
}

wire_enum! {
    /// How an option series expires.
    ///
    /// Observed values come from a captured futures option chain.
    ExpirationType {
        Regular => "Regular",
        Weekly => "Weekly",
        Quarterly => "Quarterly",
        EndOfMonth => "End-Of-Month",
    }
}

wire_enum! {
    /// When an option settles.
    SettlementType {
        Am => "AM",
        Pm => "PM",
    }
}

wire_enum! {
    /// When an option may be exercised.
    ///
    /// Only `American` appears in the captured payloads; `European` is listed
    /// because it is the other half of a two-value domain, and being wrong
    /// about it costs nothing — an unseen value round-trips through `Unknown`.
    ExerciseStyle {
        American => "American",
        European => "European",
    }
}

wire_enum! {
    /// How hard an equity is to borrow, which is what decides whether it can
    /// be shorted and at what rate.
    ///
    /// The three values are quoted verbatim from the `lendability` parameter's
    /// own description in the Instruments OpenAPI document, so this is not the
    /// usual case of guessing a variant set from a field name. `Easy To Borrow`
    /// also appears in the captured equity payload in `Doc/`.
    ///
    /// It is the same type on both sides of the wire on purpose: a value read
    /// off an [`crate::types::instrument::EquityInstrument`] can be handed
    /// straight back to a listing filter without going through a string.
    Lendability {
        EasyToBorrow => "Easy To Borrow",
        LocateRequired => "Locate Required",
        Preborrow => "Preborrow",
    }
}

#[cfg(test)]
mod wire_enum_tests {
    use super::*;

    #[test]
    fn a_known_value_maps_to_its_variant() {
        let parsed: ExpirationType = serde_json::from_str(r#""End-Of-Month""#).expect("parses");
        assert_eq!(parsed, ExpirationType::EndOfMonth);
        assert!(parsed.is_known());
        assert_eq!(parsed.to_string(), "End-Of-Month");
    }

    /// The whole point: the broker adds values, and one unseen value must not
    /// cost the caller the instrument it was attached to.
    #[test]
    fn an_unseen_value_is_preserved_rather_than_fatal() {
        let parsed: ExpirationType = serde_json::from_str(r#""Fortnightly""#)
            .expect("an unrecognised value must not fail the response");

        assert_eq!(parsed, ExpirationType::Unknown("Fortnightly".to_string()));
        assert!(!parsed.is_known(), "a caller can see this is new");
        assert_eq!(parsed.as_wire(), "Fortnightly");
    }

    /// Each enum is exercised with its own values. Round-tripping every
    /// string through one type would have "AM" and "American" taking the
    /// Unknown path, which proves nothing about the other two enums.
    #[test]
    fn every_value_round_trips_unchanged() {
        macro_rules! round_trip {
            ($ty:ty, $($text:literal),+) => {
                $(
                    let parsed: $ty = serde_json::from_str($text).expect("parses");
                    assert_eq!(
                        serde_json::to_string(&parsed).expect("serializes"),
                        $text,
                        concat!("the venue's own text must survive for ", stringify!($ty))
                    );
                )+
            };
        }

        round_trip!(
            ExpirationType,
            "\"Regular\"",
            "\"Weekly\"",
            "\"End-Of-Month\"",
            "\"Fortnightly\""
        );
        round_trip!(SettlementType, "\"AM\"", "\"PM\"", "\"Overnight\"");
        round_trip!(
            ExerciseStyle,
            "\"American\"",
            "\"European\"",
            "\"Bermudan\""
        );
    }

    /// Known values are recognised through incidental whitespace and come back
    /// canonical; unknown ones keep the venue's text exactly as it arrived.
    #[test]
    fn whitespace_does_not_hide_a_known_value() {
        let padded: SettlementType = serde_json::from_str(r#"" PM ""#).expect("parses");
        assert_eq!(padded, SettlementType::Pm);
        assert!(padded.is_known());
        assert_eq!(
            serde_json::to_string(&padded).expect("serializes"),
            r#""PM""#,
            "a known value normalises"
        );

        let unknown: SettlementType = serde_json::from_str(r#"" Overnight ""#).expect("parses");
        assert_eq!(
            unknown.as_wire(),
            " Overnight ",
            "an unknown value keeps exactly what arrived"
        );
    }

    #[test]
    fn matching_is_exhaustive_without_a_wildcard_on_known_values() {
        let settlement = SettlementType::from("PM".to_string());
        let described = match settlement {
            SettlementType::Am => "morning",
            SettlementType::Pm => "afternoon",
            SettlementType::Unknown(_) => "unrecognised",
        };
        assert_eq!(described, "afternoon");

        assert_eq!(
            ExerciseStyle::from("American".to_string()),
            ExerciseStyle::American
        );
        assert!(!ExerciseStyle::from("Bermudan".to_string()).is_known());
    }
}
