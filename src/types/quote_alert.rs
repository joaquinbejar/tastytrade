//! Quote alerts, as the account streamer and the REST endpoints both see them.
//!
//! A quote alert is a threshold a customer set on a symbol; the venue fires it
//! and publishes it over the account websocket to anyone subscribed with
//! `quote-alerts-subscribe`. The same object is what `GET /quote-alerts`
//! returns, so the type lives here rather than in the streaming module: when
//! the REST side lands (#81) it reuses this rather than declaring a second
//! shape for the same wire object.
//!
//! Alerts are per **user**, not per account, which is why nothing here carries
//! an account number.

use chrono::{DateTime, FixedOffset};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use serde::{Deserializer, Serializer};

use crate::api::quote_streaming::DxFeedSymbol;
use crate::types::order::Symbol;
use crate::types::wire::wire_enum;

wire_enum! {
    /// Which quoted value an alert watches.
    ///
    /// The four values the venue enumerates for the create body. An
    /// `Unknown(String)` arm because this type is on the **read** side too —
    /// `Items<T>` skips what it cannot parse, so a field the venue adds later
    /// would make the alert carrying it vanish from a listing.
    QuoteAlertField {
        Last => "Last",
        Bid => "Bid",
        Ask => "Ask",
        ImpliedVolatility => "IV",
    }
}

wire_enum! {
    /// How the quoted value is compared against the threshold.
    ///
    /// The venue enumerates `>` and `<` and nothing else — no `>=`, no `<=`.
    QuoteAlertOperator {
        Above => ">",
        Below => "<",
    }
}

/// A quote alert the customer configured, as the venue reports it.
///
/// Every field is `Option`: the published schema marks none of them required,
/// and the same object describes an alert that has fired, one that has
/// expired, and one that is merely waiting — so which timestamps are present
/// is exactly what tells them apart.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteAlert {
    /// The venue's identifier for this alert.
    #[serde(default)]
    pub alert_external_id: Option<String>,
    /// The user the alert belongs to. Alerts are per user, not per account.
    #[serde(default)]
    pub user_external_id: Option<String>,
    /// The instrument being watched.
    #[serde(default)]
    pub symbol: Option<Symbol>,
    /// The same instrument as the streaming feed names it.
    #[serde(default)]
    pub dx_symbol: Option<DxFeedSymbol>,
    /// The instrument type of `symbol`.
    ///
    /// `String`, not the crate's [`InstrumentType`](crate::InstrumentType),
    /// and deliberately. That enum is a closed set with no unknown arm, so a
    /// value outside it fails the whole struct — and losing an entire alert
    /// because the venue named an instrument type this crate has not seen is
    /// a bad trade for a field most callers only display. No captured frame
    /// establishes the set the alerts endpoint actually uses; convert it once
    /// `/smoke` records one, or once `InstrumentType` grows an unknown arm.
    #[serde(default)]
    pub instrument_type: Option<String>,
    /// Which quoted value the threshold applies to.
    #[serde(default)]
    pub field: Option<QuoteAlertField>,
    /// How the value is compared against the threshold.
    #[serde(default)]
    pub operator: Option<QuoteAlertOperator>,
    /// The threshold as the venue renders it for display.
    #[serde(default)]
    pub threshold: Option<String>,
    /// The threshold as a number.
    ///
    /// `Decimal`, not `f64`: it is a price, and the `f64` exemption in this
    /// crate covers the DXFeed event types only.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub threshold_numeric: Option<Decimal>,
    /// Who publishes the quotes the alert is evaluated against.
    #[serde(default)]
    pub provider: Option<String>,
    /// When the alert was created.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub created_at: Option<DateTime<FixedOffset>>,
    /// When the alert fired.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub triggered_at: Option<DateTime<FixedOffset>>,
    /// When the alert finished processing.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub completed_at: Option<DateTime<FixedOffset>>,
    /// When the customer dismissed it.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub dismissed_at: Option<DateTime<FixedOffset>>,
    /// When the alert expired.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub expired_at: Option<DateTime<FixedOffset>>,
    /// When the alert is due to expire.
    ///
    /// `String`, and tolerant of both wire shapes. The published schema types
    /// this as a string with no format, while the tastyware Python SDK — built
    /// against real frames — types it as an integer. Two sources, two shapes,
    /// and no captured frame here to settle it, so this keeps whatever arrived
    /// rather than picking one and failing to decode the alert whenever the
    /// other turns up. Its neighbours are all `date-time` and are typed.
    #[serde(default, with = "crate::types::wire::loose_string_option")]
    pub expires_at: Option<String>,
}

/// A quote alert to create.
///
/// The four fields the venue marks required are not `Option`, so a request
/// missing one cannot be built. `threshold` is the venue's own string form and
/// `threshold_numeric` is the optional numeric one it also accepts.
#[derive(DebugPretty, DisplaySimple, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct NewQuoteAlert {
    /// The instrument to watch.
    pub symbol: Symbol,
    /// Which quoted value.
    pub field: QuoteAlertField,
    /// How to compare it.
    pub operator: QuoteAlertOperator,
    /// The threshold, as the venue renders it.
    ///
    /// Private with the rest of the threshold pair: the two forms have to
    /// agree, and public fields let a caller change one and leave the other,
    /// which is an alert that fires at a price nobody asked for. Read them
    /// through [`NewQuoteAlert::threshold`].
    threshold: String,
    /// The threshold as a number.
    ///
    /// **Serialized as a string**, which is how the create schema types it —
    /// the name says numeric, the wire type does not. Sending a JSON number
    /// here was this crate's shape rather than the venue's.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::types::wire::decimal_string_option::serialize"
    )]
    threshold_numeric: Option<Decimal>,
    /// The instrument type, when the caller knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument_type: Option<String>,
    /// The streaming symbol, when the caller knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx_symbol: Option<DxFeedSymbol>,
    /// When the alert should expire, in the venue's own form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

impl NewQuoteAlert {
    /// An alert on `symbol` when `field` crosses `threshold`.
    ///
    /// `threshold` is taken as a [`Decimal`] and rendered into both the string
    /// and the numeric field, so the two cannot disagree — which they could if
    /// a caller filled them in separately, and a threshold that disagrees with
    /// itself is an alert that fires at the wrong price.
    pub fn new(
        symbol: impl Into<Symbol>,
        field: QuoteAlertField,
        operator: QuoteAlertOperator,
        threshold: Decimal,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            field,
            operator,
            threshold: threshold.to_string(),
            threshold_numeric: Some(threshold),
            instrument_type: None,
            dx_symbol: None,
            expires_at: None,
        }
    }

    /// The threshold this alert fires at.
    ///
    /// One value, because the two wire fields are derived from it and cannot
    /// drift apart.
    pub fn threshold(&self) -> Option<Decimal> {
        self.threshold_numeric
    }

    /// The threshold as the venue will see it.
    pub fn threshold_text(&self) -> &str {
        &self.threshold
    }

    /// Names the instrument type.
    #[must_use]
    pub fn with_instrument_type(mut self, instrument_type: impl Into<String>) -> Self {
        self.instrument_type = Some(instrument_type.into());
        self
    }

    /// Names the streaming symbol.
    #[must_use]
    pub fn with_dx_symbol(mut self, dx_symbol: impl Into<String>) -> Self {
        self.dx_symbol = Some(DxFeedSymbol(dx_symbol.into()));
        self
    }

    /// Sets when the alert expires, in the venue's own form.
    #[must_use]
    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }

    /// Fails when the alert cannot be what the venue accepts.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        // The two enums are tolerant because they are read-side types as well,
        // and `Items<T>` would drop an alert carrying a value this crate does
        // not model. The create schema closes both sets, so an `Unknown` here
        // is a value the venue will reject — after it has been sent, with an
        // alert the caller believes exists.
        if !self.field.is_known() {
            return Err(crate::TastyTradeError::Precondition(format!(
                "{} is not a field the venue accepts on a new alert; it takes Last, \
                 Bid, Ask or IV",
                self.field.as_wire()
            )));
        }
        if !self.operator.is_known() {
            return Err(crate::TastyTradeError::Precondition(format!(
                "{} is not an operator the venue accepts on a new alert; it takes \
                 > or <",
                self.operator.as_wire()
            )));
        }
        if self.symbol.0.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a quote alert needs a symbol, and this one is blank".to_string(),
            ));
        }
        if self.threshold.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a quote alert needs a threshold".to_string(),
            ));
        }
        // A threshold of zero on a price is almost always a caller who forgot
        // to set it, and an alert that fires immediately is worse than one that
        // is refused.
        if self.threshold_numeric == Some(Decimal::ZERO) {
            return Err(crate::TastyTradeError::Precondition(
                "a quote alert threshold of zero would fire immediately on any \
                 quote; set the price you mean"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// The shape the venue documents, with every field present.
    #[test]
    fn a_triggered_alert_decodes() {
        let frame = r#"{
            "alert-external-id": "alert-1",
            "user-external-id": "U0001",
            "symbol": "AAPL",
            "dx-symbol": "AAPL",
            "instrument-type": "Equity",
            "field": "Last",
            "operator": ">",
            "threshold": "200.00",
            "threshold-numeric": 200.0,
            "provider": "dxfeed",
            "created-at": "2026-08-01T12:00:00.000+00:00",
            "triggered-at": "2026-08-02T14:30:00.000-04:00",
            "expires-at": "1788595114405"
        }"#;

        let alert: QuoteAlert = serde_json::from_str(frame).expect("a documented alert decodes");

        assert_eq!(alert.symbol.map(|s| s.0), Some("AAPL".to_string()));
        assert_eq!(
            alert.threshold_numeric,
            Some(Decimal::from_str("200.0").expect("a decimal"))
        );
        // The offset is kept rather than normalised: a market event happened at
        // a local time, and that is information.
        let triggered = alert.triggered_at.expect("it fired");
        assert_eq!(triggered.offset().local_minus_utc(), -4 * 3600);
        assert!(alert.dismissed_at.is_none());
    }

    /// An alert that has not fired sends almost nothing. A required field here
    /// would mean the notification could not be delivered at all.
    #[test]
    fn a_sparse_alert_still_decodes() {
        let alert: QuoteAlert =
            serde_json::from_str(r#"{"alert-external-id":"alert-2"}"#).expect("sparse is valid");

        assert_eq!(alert.alert_external_id.as_deref(), Some("alert-2"));
        assert!(alert.symbol.is_none());
        assert!(alert.threshold_numeric.is_none());
        assert!(alert.expires_at.is_none());
    }

    /// The two sources disagree about `expires-at`. Both shapes have to
    /// survive, or the alert stops decoding the day the venue picks the other.
    #[test]
    fn expires_at_accepts_both_shapes_the_sources_disagree_about() {
        let as_number: QuoteAlert =
            serde_json::from_str(r#"{"expires-at":1788595114405}"#).expect("a number decodes");
        assert_eq!(as_number.expires_at.as_deref(), Some("1788595114405"));

        let as_text: QuoteAlert = serde_json::from_str(r#"{"expires-at":"2026-09-01T00:00:00Z"}"#)
            .expect("a string decodes");
        assert_eq!(as_text.expires_at.as_deref(), Some("2026-09-01T00:00:00Z"));
    }

    /// A price is `Decimal` everywhere outside `types::dxfeed`, and a
    /// threshold arriving as a quoted string must not be the exception.
    #[test]
    fn a_quoted_threshold_keeps_its_digits() {
        let alert: QuoteAlert = serde_json::from_str(r#"{"threshold-numeric":"1234.56789"}"#)
            .expect("a quoted decimal decodes");
        assert_eq!(
            alert.threshold_numeric,
            Some(Decimal::from_str("1234.56789").expect("a decimal"))
        );
    }
}

#[cfg(test)]
mod create_tests {
    use super::*;
    use std::str::FromStr;

    fn alert() -> NewQuoteAlert {
        NewQuoteAlert::new(
            "AAPL",
            QuoteAlertField::Last,
            QuoteAlertOperator::Above,
            Decimal::from_str("200.00").expect("a price"),
        )
    }

    /// The two threshold fields are derived from one argument, so they cannot
    /// disagree — and a threshold that disagrees with itself is an alert that
    /// fires at the wrong price.
    #[test]
    fn both_threshold_forms_come_from_one_value() {
        let body = serde_json::to_value(alert()).expect("serialises");

        assert_eq!(body["threshold"], "200.00");
        // A **string**, which is how the create schema types this field
        // despite its name. Arbitrary precision keeps the trailing zeros the
        // caller wrote, so it is `200.00` and not a rounded `200.0`.
        assert_eq!(body["threshold-numeric"], "200.00");
        assert!(
            body["threshold-numeric"].is_string(),
            "the create schema types threshold-numeric as a string: {body}"
        );
        assert_eq!(body["symbol"], "AAPL");
        assert_eq!(body["field"], "Last");
        assert_eq!(body["operator"], ">");

        // And the two forms cannot drift apart, because there is one value
        // behind them and no way to set either directly.
        let alert = alert();
        assert_eq!(alert.threshold_text(), "200.00");
        assert_eq!(
            alert.threshold().map(|t| t.to_string()),
            Some("200.00".into())
        );
    }

    /// The create schema closes both enums, so a value this crate decoded from
    /// a response but does not model cannot be sent back on a new alert.
    ///
    /// The types stay tolerant because they are read-side too: `Items<T>` drops
    /// what it cannot parse, and an alert vanishing from a listing is worse
    /// than one carrying an unfamiliar field name. Sending one is a request the
    /// venue rejects, after this crate has told the caller it was made.
    #[test]
    fn an_unmodelled_field_or_operator_cannot_be_created() {
        let mut unknown_field = alert();
        unknown_field.field = QuoteAlertField::from("Theta".to_string());
        let error = unknown_field
            .validate()
            .expect_err("Theta is not a field the venue accepts");
        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable(), "nothing was sent");

        let mut unknown_operator = alert();
        unknown_operator.operator = QuoteAlertOperator::from(">=".to_string());
        assert!(unknown_operator.validate().is_err(), ">= is not < or >");

        // A modelled pair still goes.
        alert().validate().expect("Last and > are both modelled");

        // …and both still decode, which is the half that has to stay tolerant.
        let listed: QuoteAlert = serde_json::from_str(r#"{"field": "Theta", "operator": ">="}"#)
            .expect("an alert must not vanish because of an unfamiliar value");
        assert_eq!(
            listed.field.map(|f| f.as_wire().to_string()),
            Some("Theta".into())
        );
    }

    /// The optional fields are omitted rather than sent as null.
    #[test]
    fn unset_optional_fields_are_omitted() {
        let body = serde_json::to_value(alert()).expect("serialises");

        assert!(body.get("instrument-type").is_none(), "{body}");
        assert!(body.get("dx-symbol").is_none(), "{body}");
        assert!(body.get("expires-at").is_none(), "{body}");

        let full = serde_json::to_value(
            alert()
                .with_instrument_type("Equity")
                .with_dx_symbol("AAPL")
                .with_expires_at("1788595114405"),
        )
        .expect("serialises");
        assert_eq!(full["instrument-type"], "Equity");
        assert_eq!(full["dx-symbol"], "AAPL");
        assert_eq!(full["expires-at"], "1788595114405");
    }

    #[test]
    fn a_blank_symbol_is_refused_locally() {
        let mut blank = alert();
        blank.symbol = Symbol("   ".to_string());

        let error = blank.validate().expect_err("a blank symbol is refused");
        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(!error.is_retryable());
    }

    /// A threshold of zero fires on the first quote, which is almost always a
    /// caller who forgot to set it.
    #[test]
    fn a_zero_threshold_is_refused_locally() {
        let zero = NewQuoteAlert::new(
            "AAPL",
            QuoteAlertField::Bid,
            QuoteAlertOperator::Below,
            Decimal::ZERO,
        );

        assert!(zero.validate().is_err());
    }

    #[test]
    fn a_well_formed_alert_is_accepted() {
        assert!(alert().validate().is_ok());
    }

    /// The operators are the venue's own two, and nothing else.
    #[test]
    fn the_operator_uses_the_venues_spelling() {
        assert_eq!(QuoteAlertOperator::Above.as_wire(), ">");
        assert_eq!(QuoteAlertOperator::Below.as_wire(), "<");
        assert_eq!(QuoteAlertField::ImpliedVolatility.as_wire(), "IV");
    }

    /// A field the venue adds later keeps its text rather than making the
    /// alert vanish from a listing through `Items<T>`.
    #[test]
    fn an_unrecognised_field_survives_on_the_read_side() {
        let alert: QuoteAlert =
            serde_json::from_str(r#"{"alert-external-id": "a", "field": "Theta"}"#)
                .expect("the alert must still decode");

        assert_eq!(
            alert.field,
            Some(QuoteAlertField::Unknown("Theta".to_string()))
        );
    }
}
