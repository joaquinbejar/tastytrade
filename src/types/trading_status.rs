//! Whether an account may trade, and what it may trade.
//!
//! The cheap question to ask before an order: a closed or frozen account cannot
//! trade at all, a "closing only" account can only reduce, and the feature
//! flags decide whether futures, cryptocurrency or uncovered short calls are
//! available. Without this a caller discovers a restriction when the venue
//! rejects the order.
//!
//! Every field is `Option<T>`, per the `AccountDetails` precedent: a flag the
//! broker did not send is **unknown**, never `false`. Certification is known to
//! omit fields production sends, and "we were not told whether this account is
//! frozen" and "this account is not frozen" are different facts — only one of
//! them is safe to act on.

use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::accounts::AccountNumber;

/// What an account is allowed to do right now.
///
/// `Debug` and `Display` render the status with both account identifiers
/// replaced. The derives went through `Serialize`, so a `{status:?}` in a log
/// line printed them — and the `tracing` macros do exactly that. Serializing
/// keeps the real values: writing the record out is an explicit act.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TradingStatus {
    /// Which account this is about. Account PII.
    #[serde(default)]
    pub account_number: Option<AccountNumber>,
    /// Whether deep in-the-money options may be carried.
    #[serde(default)]
    pub are_deep_itm_carry_options_enabled: Option<bool>,
    /// Whether far out-of-the-money net options are restricted.
    #[serde(default)]
    pub are_far_otm_net_options_restricted: Option<bool>,
    /// Whether option values are capped at net liquidating value.
    #[serde(default)]
    pub are_options_values_restricted_to_nlv: Option<bool>,
    /// Whether single-tick expiring hedges are ignored.
    #[serde(default)]
    pub are_single_tick_expiring_hedges_ignored: Option<bool>,
    /// The autotrade classification, when the account has one.
    #[serde(default)]
    pub autotrade_account_type: Option<String>,
    /// The clearing firm's account number. Account PII.
    #[serde(default)]
    pub clearing_account_number: Option<AccountNumber>,
    /// How the clearing firm aggregates this account.
    #[serde(default)]
    pub clearing_aggregation_identifier: Option<String>,
    /// CMTA override, when one is set.
    #[serde(default)]
    pub cmta_override: Option<i64>,
    /// Day trades used so far today. Updated live through the session.
    #[serde(default)]
    pub day_trade_count: Option<i64>,
    /// When enhanced fraud safeguards were switched on, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub enhanced_fraud_safeguards_enabled_at: Option<DateTime<FixedOffset>>,
    /// How equity margin is calculated, e.g. `Reg T`.
    #[serde(default)]
    pub equities_margin_calculation_type: Option<String>,
    /// External CRM identifier.
    #[serde(default)]
    pub ext_crm_id: Option<String>,
    /// Which fee schedule the account is on.
    #[serde(default)]
    pub fee_schedule_name: Option<String>,
    /// Multiplier applied to futures margin rates.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub futures_margin_rate_multiplier: Option<Decimal>,
    /// Whether intraday equity margin applies.
    #[serde(default)]
    pub has_intraday_equities_margin: Option<bool>,
    /// The venue's identifier for this status record.
    #[serde(default)]
    pub id: Option<i64>,
    /// Whether the account is aggregated at the clearing firm.
    #[serde(default)]
    pub is_aggregated_at_clearing: Option<bool>,
    /// Whether Ascend event contracts are enabled.
    #[serde(default)]
    pub is_ascend_event_contracts_enabled: Option<bool>,
    /// Whether the account is closed. A closed account cannot trade.
    #[serde(default)]
    pub is_closed: Option<bool>,
    /// Whether only closing trades are permitted.
    #[serde(default)]
    pub is_closing_only: Option<bool>,
    /// Whether only closing cryptocurrency trades are permitted.
    #[serde(default)]
    pub is_cryptocurrency_closing_only: Option<bool>,
    /// Whether cryptocurrency trading is enabled for the account.
    #[serde(default)]
    pub is_cryptocurrency_enabled: Option<bool>,
    /// Whether only closing equity-offering trades are permitted.
    #[serde(default)]
    pub is_equity_offering_closing_only: Option<bool>,
    /// Whether equity offerings are enabled.
    #[serde(default)]
    pub is_equity_offering_enabled: Option<bool>,
    /// Whether only closing event-contract trades are permitted.
    #[serde(default)]
    pub is_event_contracts_closing_only: Option<bool>,
    /// Whether the account is frozen. A frozen account cannot trade.
    #[serde(default)]
    pub is_frozen: Option<bool>,
    /// Whether full equity margin is required.
    #[serde(default)]
    pub is_full_equity_margin_required: Option<bool>,
    /// Whether only closing futures trades are permitted.
    #[serde(default)]
    pub is_futures_closing_only: Option<bool>,
    /// Whether futures trading is enabled.
    #[serde(default)]
    pub is_futures_enabled: Option<bool>,
    /// Whether intraday futures margin is enabled.
    #[serde(default)]
    pub is_futures_intra_day_enabled: Option<bool>,
    /// Whether the account is in a day-trade equity maintenance call.
    #[serde(default)]
    pub is_in_day_trade_equity_maintenance_call: Option<bool>,
    /// Whether the account is in a margin call.
    #[serde(default)]
    pub is_in_margin_call: Option<bool>,
    /// Whether the account is classified as non-retail.
    #[serde(default)]
    pub is_non_retail: Option<bool>,
    /// Whether the account is flagged as a pattern day trader.
    #[serde(default)]
    pub is_pattern_day_trader: Option<bool>,
    /// Whether portfolio margin is enabled.
    #[serde(default)]
    pub is_portfolio_margin_enabled: Option<bool>,
    /// Whether only risk-reducing trades are permitted.
    #[serde(default)]
    pub is_risk_reducing_only: Option<bool>,
    /// Whether roll-the-day-forward is enabled.
    #[serde(default)]
    pub is_roll_the_day_forward_enabled: Option<bool>,
    /// Whether intraday small-notional futures are enabled.
    #[serde(default)]
    pub is_small_notional_futures_intra_day_enabled: Option<bool>,
    /// What the account may do with options, e.g. `No Restrictions`.
    #[serde(default)]
    pub options_level: Option<String>,
    /// The day the pattern-day-trader flag resets, when one is set.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub pdt_reset_on: Option<NaiveDate>,
    /// Whether uncovered short calls are permitted.
    #[serde(default)]
    pub short_calls_enabled: Option<bool>,
    /// Multiplier applied to small-notional futures margin rates.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub small_notional_futures_margin_rate_multiplier: Option<Decimal>,
    /// When the status was last updated, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub updated_at: Option<DateTime<FixedOffset>>,
}

impl TradingStatus {
    /// Whether the venue has said the account is blocked from trading.
    ///
    /// `true` only when a blocking flag actually arrived and was set. A flag
    /// the venue omitted is unknown, so a `false` here is **not** a licence to
    /// trade — pair it with [`TradingStatus::is_known_blocked`] when the
    /// difference matters, which on a money path it does.
    pub fn is_blocked(&self) -> bool {
        self.is_closed == Some(true) || self.is_frozen == Some(true)
    }

    /// Whether every flag [`TradingStatus::is_blocked`] reads actually arrived.
    ///
    /// Exists so a caller can tell "the account is fine" from "the venue did
    /// not say". `Option<bool>` preserves that per field, and a convenience
    /// method that collapsed it would undo the whole point.
    pub fn is_known_blocked(&self) -> bool {
        self.is_closed.is_some() && self.is_frozen.is_some()
    }

    /// Whether only position-reducing trades are permitted, as far as the
    /// venue said.
    pub fn is_reduce_only(&self) -> bool {
        self.is_closing_only == Some(true) || self.is_risk_reducing_only == Some(true)
    }

    /// Whether every flag [`TradingStatus::is_reduce_only`] reads arrived.
    ///
    /// The companion [`TradingStatus::is_known_blocked`] is to
    /// [`TradingStatus::is_blocked`]. Without it, `false` from a sparse
    /// payload said "the account may trade freely" when what the venue
    /// actually said was nothing — and this is the flag that decides whether
    /// an order will be accepted.
    pub fn is_known_reduce_only(&self) -> bool {
        self.is_closing_only.is_some() && self.is_risk_reducing_only.is_some()
    }
}

crate::types::wire::redacted_account_render!(TradingStatus);

#[cfg(test)]
mod tests {
    use super::*;

    /// The payload from the venue's own guide, account numbers redacted.
    const FIXTURE: &str = include_str!("../../Doc/trading_status.json");

    fn status() -> TradingStatus {
        let body: serde_json::Value = serde_json::from_str(FIXTURE).expect("valid JSON");
        serde_json::from_value(body["data"].clone()).expect("the status must decode")
    }

    #[test]
    fn the_venues_own_payload_decodes() {
        let status = status();

        assert_eq!(status.day_trade_count, Some(0));
        assert_eq!(
            status.equities_margin_calculation_type.as_deref(),
            Some("Reg T")
        );
        assert_eq!(status.options_level.as_deref(), Some("No Restrictions"));
        assert_eq!(status.is_cryptocurrency_enabled, Some(true));
        assert_eq!(status.is_futures_enabled, Some(true));
        assert_eq!(status.short_calls_enabled, Some(true));
    }

    /// The multipliers are ratios, so `Decimal` and not `f64`, and they arrive
    /// quoted.
    #[test]
    fn the_margin_multipliers_are_decimal() {
        let status = status();

        assert_eq!(
            status
                .futures_margin_rate_multiplier
                .expect("a multiplier")
                .to_string(),
            "0.0"
        );
        assert_eq!(
            status
                .small_notional_futures_margin_rate_multiplier
                .expect("a multiplier")
                .to_string(),
            "0.0"
        );
    }

    #[test]
    fn timestamps_keep_their_offset() {
        let status = status();

        assert_eq!(
            status
                .updated_at
                .expect("an updated-at")
                .offset()
                .local_minus_utc(),
            0
        );
        assert!(status.enhanced_fraud_safeguards_enabled_at.is_some());
    }

    /// The `AccountDetails` precedent, and the case where defaulting to
    /// `false` would tell a caller an account is tradable when nobody said so.
    #[test]
    fn a_flag_the_venue_omitted_is_unknown_and_not_false() {
        let sparse: TradingStatus =
            serde_json::from_str(r#"{"account-number": "SENTINEL-5WX00042"}"#)
                .expect("a thin payload must still decode");

        assert_eq!(sparse.is_frozen, None);
        assert_eq!(sparse.is_closed, None);
        assert_eq!(sparse.is_cryptocurrency_enabled, None);

        assert!(!sparse.is_blocked());
        assert!(
            !sparse.is_known_blocked(),
            "nothing was said, so nothing is known"
        );

        // The same distinction on the flag that decides whether an order will
        // be accepted. `false` from a sparse payload used to be the only
        // answer available, and it read as "may trade freely".
        assert_eq!(sparse.is_closing_only, None);
        assert_eq!(sparse.is_risk_reducing_only, None);
        assert!(!sparse.is_reduce_only());
        assert!(
            !sparse.is_known_reduce_only(),
            "nothing was said, so nothing is known"
        );
    }

    /// Neither account identifier reaches a rendering.
    ///
    /// The derives went through `Serialize`, so both printed verbatim as soon
    /// as anything wrote `{status:?}` — which is what a `tracing` field does.
    /// `AccountNumber` alone does not fix it: it serializes transparently.
    #[test]
    fn a_trading_status_renders_without_either_account_identifier() {
        const ACCOUNT: &str = "SENTINEL-5WX00042";
        const CLEARING: &str = "SENTINEL-clearing-99887";

        let status: TradingStatus = serde_json::from_str(&format!(
            r#"{{"account-number": "{ACCOUNT}", "clearing-account-number": "{CLEARING}",
                 "is-frozen": false, "options-level": "No Restrictions"}}"#
        ))
        .expect("the status must decode");

        let rendered = format!("{status:?} {status} {}", format_args!("{status:#?}"));
        assert!(!rendered.contains(ACCOUNT), "rendered: {rendered}");
        assert!(!rendered.contains(CLEARING), "rendered: {rendered}");
        assert!(rendered.contains("{account}"), "{rendered}");
        // The rest of the status is still readable, which is the point of
        // redacting the identifiers rather than the record.
        assert!(rendered.contains("No Restrictions"), "{rendered}");

        // Serialization is explicit and keeps both.
        let written = serde_json::to_string(&status).expect("must serialize");
        assert!(written.contains(ACCOUNT) && written.contains(CLEARING));
    }

    #[test]
    fn a_blocked_account_reports_itself_blocked() {
        let mut status = status();
        assert!(!status.is_blocked());
        assert!(status.is_known_blocked(), "the venue sent both flags");

        status.is_frozen = Some(true);
        assert!(status.is_blocked());

        status.is_frozen = Some(false);
        status.is_closing_only = Some(true);
        assert!(!status.is_blocked(), "closing-only can still trade");
        assert!(status.is_reduce_only());
    }
}
