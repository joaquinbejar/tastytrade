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

/// A decimal the venue types as a **string** on the wire.
///
/// [`decimal_option`] accepts both shapes coming in and writes a JSON number
/// going out, which is what the order bodies already used. A few request
/// schemas type the field as a string instead — `threshold-numeric` on a quote
/// alert is one, despite the name — and sending a number there is this crate's
/// shape rather than the venue's. Reading is unchanged and still tolerant of
/// both, because a response is not the caller's problem to get right.
pub(crate) mod decimal_string_option {
    use super::*;

    pub(crate) fn serialize<S>(value: &Option<Decimal>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_str(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }
}

/// A decimal the venue sometimes reports as the literal string `NaN`.
///
/// A fixing price that does not apply to an instrument comes back as `"NaN"`,
/// which is not a number and never will be one. `Decimal` has no such value —
/// it is a fixed-point type, which is the whole reason money uses it — so the
/// only faithful answer is `None`.
///
/// Deliberately narrow: **only** `NaN` maps to `None`. Anything else
/// unparseable is still an error, because a helper that swallowed every bad
/// value would turn a decoding bug into a silently absent price on a margin
/// report.
pub(crate) mod decimal_option_nan {
    use super::*;

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<WireNumber>::deserialize(deserializer)? {
            Some(WireNumber::Text(text)) if text.trim().eq_ignore_ascii_case("nan") => Ok(None),
            Some(raw) => raw.into_decimal().map(Some),
            None => Ok(None),
        }
    }

    pub(crate) fn serialize<S>(value: &Option<Decimal>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        super::decimal_option::serialize(value, serializer)
    }
}

/// A map of symbol to price, as the margin report sends its marks.
pub(crate) mod decimal_map {
    use super::*;
    use std::collections::HashMap;

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<HashMap<String, Decimal>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = match Option::<HashMap<String, WireNumber>>::deserialize(deserializer)? {
            Some(raw) => raw,
            None => return Ok(None),
        };

        let mut out = HashMap::with_capacity(raw.len());
        for (symbol, number) in raw {
            out.insert(symbol, number.into_decimal()?);
        }
        Ok(Some(out))
    }

    pub(crate) fn serialize<S>(
        value: &Option<HashMap<String, Decimal>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(map) => serde::Serialize::serialize(map, serializer),
            None => serializer.serialize_none(),
        }
    }
}

/// A calendar day the venue's schema types as a timestamp.
///
/// An option expiration is a day of market, not an instant — that is the rule
/// the rest of this crate follows and the reason `Expiration` uses
/// [`chrono::NaiveDate`]. The market-metrics schema types its expiration as
/// `date-time` anyway, so this accepts either shape and keeps the day.
///
/// Deliberately narrow: a plain `YYYY-MM-DD` or an RFC 3339 timestamp, nothing
/// else. Taking the date out of a timestamp discards a time-of-day the venue
/// never meant as one; inventing a timezone to keep it would be worse, and
/// picking one shape would make every row fail the first time the other
/// arrived.
pub(crate) mod expiration_date_option {
    use super::*;
    use chrono::{DateTime, NaiveDate};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(text) = Option::<String>::deserialize(deserializer)? else {
            return Ok(None);
        };
        let text = text.trim();

        if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            return Ok(Some(date));
        }

        DateTime::parse_from_rfc3339(text)
            .map(|moment| Some(moment.date_naive()))
            .map_err(|e| {
                serde::de::Error::custom(format!(
                    "expected an expiration as YYYY-MM-DD or RFC 3339 ({e})"
                ))
            })
    }

    pub(crate) fn serialize<S>(value: &Option<NaiveDate>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        super::date_option::serialize(value, serializer)
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

// `macro_rules!` is textually scoped, so a macro defined here is invisible to
// sibling modules without this. Re-exported at crate level rather than
// `#[macro_export]`ed: the macro generates a **public** enum, and exporting it
// from the crate root would make it part of the public API by accident.
pub(crate) use wire_enum;

/// Decodes a field the venue may answer with a value this crate does not model.
///
/// The row survives; the field becomes `None`. Without this a single
/// unrecognised value fails the whole struct, and [`crate::api::base::Items`]
/// drops a struct that fails — so a ledger quietly loses a transaction, with
/// every other field on it intact and unread.
///
/// The cost is that an unrecognised value and an absent one both read as
/// `None`. That is the lesser loss: the alternative is losing the row. The
/// rejected text goes to DEBUG so it can be identified and modelled, and never
/// higher, because it is venue data about an account's activity.
///
/// For enums whose full value set matters to the caller, [`wire_enum`] and its
/// `Unknown(String)` arm are the better tool. This is for fields typed against
/// an enum that is shared with request paths, where adding an arm would change
/// what callers are allowed to send.
pub(crate) fn tolerant_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };

    match serde_json::from_value::<T>(value.clone()) {
        Ok(decoded) => Ok(Some(decoded)),
        Err(error) => {
            tracing::debug!(
                "field value {} is not one this crate models ({}); the field reads as absent \
                 and the rest of the record is kept",
                value,
                error
            );
            Ok(None)
        }
    }
}

/// Renders a value as JSON with the identifiers a log must not carry replaced.
///
/// `DebugPretty` and `DisplaySimple` render through `Serialize`, so a struct
/// holding an account number prints it the moment anybody writes `{value:?}` —
/// including the `tracing` macros, which is how it reaches an aggregator
/// nobody meant to send it to. Redaction is a property of the type, so the
/// types that carry an account number render through this instead of deriving.
///
/// Structural rather than per-field: it walks the serialized value and replaces
/// **every** key whose name identifies an account, at any depth. A per-field
/// implementation is a per-field opportunity to forget one, and these records
/// nest — a complex order carries component orders, each with their own copy.
///
/// Serialization itself is untouched. Writing the record out is an explicit act
/// with an explicit destination; rendering it is not.
/// Whether a field or parameter name identifies an account.
///
/// Matched on the **suffix** rather than the whole name, because the venue
/// qualifies the concept: `clearing-account-number` is an account number too,
/// and an exact list missed it. A suffix rule covers the qualified spellings
/// that exist and the ones that arrive later. Both separators are accepted:
/// the same concept is `account-number` on the wire and `account_number` in a
/// derived Rust field name.
///
/// One rule, two places that need it — the JSON renderer here and the request
/// URL redaction in [`crate::api::client`], where the venue takes the same
/// identifiers as query parameters.
pub(crate) fn names_an_account(name: &str) -> bool {
    let name = name.replace('_', "-");
    // `account-numbers[]` is how the customer order endpoints spell a repeated
    // parameter, and the brackets are part of the key. The HTTP client
    // percent-encodes them on the way out, so the name reaches the redaction
    // as `account-numbers%5B%5D` — matching only the literal form would have
    // let every real request through.
    let lowered = name.to_ascii_lowercase();
    let name = lowered
        .strip_suffix("[]")
        .or_else(|| lowered.strip_suffix("%5b%5d"))
        .unwrap_or(&lowered);
    name == "account-number"
        || name == "account-numbers"
        || name.ends_with("-account-number")
        || name.ends_with("-account-numbers")
}

pub(crate) fn redacted_render(value: &impl serde::Serialize) -> String {
    use names_an_account as is_account_key;

    fn redact(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, entry) in map.iter_mut() {
                    if is_account_key(key) && !entry.is_null() {
                        *entry = serde_json::Value::String("{account}".to_string());
                    } else {
                        redact(entry);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(redact),
            _ => {}
        }
    }

    match serde_json::to_value(value) {
        Ok(mut rendered) => {
            redact(&mut rendered);
            rendered.to_string()
        }
        // A value that will not serialize has nothing safe to say about
        // itself, so it says nothing rather than falling back to a derive.
        Err(_) => "<unrenderable>".to_string(),
    }
}

/// Writes `Debug` and `Display` through [`redacted_render`].
///
/// For records that are useful to look at and carry an account number: the
/// record renders, the identifier does not.
macro_rules! redacted_account_render {
    ($name:ident) => {
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    concat!(stringify!($name), " {}"),
                    $crate::types::wire::redacted_render(self)
                )
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&$crate::types::wire::redacted_render(self))
            }
        }
    };
}

pub(crate) use redacted_account_render;

wire_enum! {
    /// The venue's `product-type` classification of a futures product.
    ///
    /// **Not a settlement flag.** `cash_settled` is its own field and carries
    /// that meaning; the captured `FutureOptionProduct` fixture pairs
    /// `"cash-settled": false` with `"product-type": "Financial"`, so reading
    /// this as cash-versus-physical settlement would contradict the record it
    /// came from. What the two values distinguish is not documented anywhere
    /// published, so this says what the venue calls them and nothing more.
    ///
    /// Observed on 2026-08-04 against certification: 83 future products carried
    /// 305 occurrences of the field between them and every one was `Financial`
    /// or `Physical`. That is a measured set rather than a guessed one, which
    /// is what kept this a `String` until now — see
    /// [#125](https://github.com/joaquinbejar/tastytrade/issues/125).
    ///
    /// It keeps the `Unknown` arm regardless: 83 products is a good sample and
    /// not a guarantee, and `Items<T>` drops a row it cannot decode.
    ProductType {
        Financial => "Financial",
        Physical => "Physical",
    }
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
