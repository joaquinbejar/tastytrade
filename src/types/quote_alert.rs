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

use crate::api::quote_streaming::DxFeedSymbol;
use crate::types::order::Symbol;

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
    #[serde(default)]
    pub instrument_type: Option<String>,
    /// Which quoted value the threshold applies to, such as `Last`.
    #[serde(default)]
    pub field: Option<String>,
    /// How the value is compared against the threshold.
    #[serde(default)]
    pub operator: Option<String>,
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
