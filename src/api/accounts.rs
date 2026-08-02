use super::base::{Items, Paginated};
use crate::api::base::TastyResult;
use crate::types::balance::{Balance, BalanceSnapshot, SnapshotTimeOfDay};
use crate::types::order::{DryRunResult, Order, OrderId, OrderPlacedResult, Warning};
use crate::{FullPosition, LiveOrderRecord, TastyTrade};
use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};

#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone,
)]
#[serde(transparent)]
pub struct AccountNumber(pub String);

impl<T: AsRef<str>> From<T> for AccountNumber {
    fn from(value: T) -> Self {
        Self(value.as_ref().to_owned())
    }
}

impl AccountNumber {
    /// A form of this account number that is safe to log.
    ///
    /// Enough of it survives to tell two accounts apart in a log or a support
    /// thread; not enough to identify the account to someone who did not
    /// already have it. Anything short enough that a prefix and a suffix would
    /// reveal most of it is masked entirely.
    ///
    /// This exists because the rule — account identifiers stay out of logs —
    /// is easy to state and easy to forget at the call site. Somewhere to
    /// reach for makes it easier to follow than to break.
    pub fn redacted(&self) -> String {
        const KEEP_PREFIX: usize = 2;
        const KEEP_SUFFIX: usize = 3;

        let chars: Vec<char> = self.0.chars().collect();
        if chars.len() <= KEEP_PREFIX + KEEP_SUFFIX {
            return "*".repeat(chars.len().max(1));
        }

        let prefix: String = chars[..KEEP_PREFIX].iter().collect();
        let suffix: String = chars[chars.len() - KEEP_SUFFIX..].iter().collect();
        format!("{prefix}…{suffix}")
    }
}

/// Details of a single trading account.
///
/// The certification environment and production do not return the same set of
/// keys, and either side gains fields over time. A strict field is not a safe
/// default here: `Items<T>` skips items it cannot parse, so one missing key
/// turns a live account into an empty list rather than an error.
///
/// Tolerance does not mean inventing an answer. A flag the venue did not send
/// is `None`, never `false` — "the broker did not say whether this account is
/// in a firm error state" and "this account is not in a firm error state" are
/// different facts, and only one of them is safe to act on.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountDetails {
    /// Broker-assigned account identifier.
    pub account_number: AccountNumber,
    /// External identifier, when the account carries one.
    pub external_id: Option<String>,
    /// Timestamp the account was opened, RFC 3339.
    #[serde(with = "crate::types::wire::datetime")]
    pub opened_at: DateTime<FixedOffset>,
    /// User-facing name of the account.
    pub nickname: String,
    /// Account type as named by the broker, e.g. `Individual`.
    pub account_type_name: String,
    /// Whether the account is flagged as a pattern day trader. `None` when the
    /// venue omits the flag, which is not the same as `false`.
    pub day_trader_status: Option<bool>,
    /// Whether the account is in a firm error state. `None` when the venue
    /// omits the flag, which is not the same as `false`.
    pub is_firm_error: Option<bool>,
    /// Whether the account is firm proprietary. `None` when the venue omits
    /// the flag, which is not the same as `false`.
    pub is_firm_proprietary: Option<bool>,
    /// Whether the account is a test-drive account.
    ///
    /// The only flag that defaults rather than reporting `None`: certification
    /// never sends it, and every account it serves is a real one from the
    /// caller's point of view, which is what `false` says.
    #[serde(default)]
    pub is_test_drive: bool,
    /// Whether the account is margin or cash.
    pub margin_or_cash: String,
    /// Whether the account is foreign. `None` when the venue omits the flag,
    /// which is not the same as `false`.
    pub is_foreign: Option<bool>,
    /// Date the account was funded, when it has been.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub funding_date: Option<NaiveDate>,
    /// Whether the account has been closed. `None` when the venue omits it.
    pub is_closed: Option<bool>,
    /// Timestamp the account record was created.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub created_at: Option<DateTime<FixedOffset>>,
    /// Stated investment objective, e.g. `SPECULATION`.
    pub investment_objective: Option<String>,
    /// Whether the account is approved to trade futures.
    pub is_futures_approved: Option<bool>,
    /// Options level the account is suitable for, e.g. `Defined Risk Spreads`.
    pub suitable_options_level: Option<String>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountInner {
    pub account: AccountDetails,
    pub authority_level: String,
}

/// Evidence that a specific order was dry-run against a specific account.
///
/// Produced only by [`Account::review_order`], so it cannot be forged by
/// constructing a value: the fields are private and there is no constructor.
/// Turning it into a [`ReviewedOrder`] is a deliberate step, and when the
/// venue attached warnings the only way through is the method that says so in
/// its name.
#[derive(Debug)]
pub struct DryRunReceipt {
    account_number: AccountNumber,
    /// The base URL the dry run was answered by.
    ///
    /// An account number is just text, and certification reuses production
    /// numbering, so binding to the number alone would let a sandbox dry run
    /// authorise a real order. The origin is what actually distinguishes the
    /// venue that gave the answer.
    origin: String,
    order: Order,
    result: DryRunResult,
}

impl DryRunReceipt {
    /// Everything the venue said about the order: buying-power effect, fees
    /// and warnings.
    pub fn result(&self) -> &DryRunResult {
        &self.result
    }

    /// The warnings the venue attached, which is the part worth reading before
    /// risking money.
    pub fn warnings(&self) -> &[Warning] {
        &self.result.warnings
    }

    /// The order this receipt is about.
    pub fn order(&self) -> &Order {
        &self.order
    }

    /// Whether the venue attached anything that needs reading.
    pub fn is_clean(&self) -> bool {
        self.result.warnings.is_empty()
    }

    /// Accepts a clean dry run.
    ///
    /// # Errors
    ///
    /// Returns [`TastyTradeError::ConfigError`] when the venue attached
    /// warnings. That is not a refusal to proceed — it is a refusal to proceed
    /// *silently*. Read [`DryRunReceipt::warnings`] first, then use
    /// [`DryRunReceipt::accept_with_warnings`] to say you did.
    pub fn accept(self) -> TastyResult<ReviewedOrder> {
        if !self.is_clean() {
            return Err(crate::TastyTradeError::Precondition(format!(
                "the venue attached {} warning(s) to this order; read them and use \
                 accept_with_warnings to proceed deliberately",
                self.result.warnings.len()
            )));
        }

        Ok(ReviewedOrder {
            account_number: self.account_number,
            origin: self.origin,
            order: self.order,
        })
    }

    /// Accepts a dry run whose warnings the caller has read.
    ///
    /// Named so that the decision is visible at the call site rather than
    /// buried in a boolean argument.
    pub fn accept_with_warnings(self) -> ReviewedOrder {
        ReviewedOrder {
            account_number: self.account_number,
            origin: self.origin,
            order: self.order,
        }
    }
}

/// An order that has been dry-run and accepted, ready for
/// [`Account::place_reviewed_order`].
///
/// Like [`DryRunReceipt`], this has no public constructor: holding one means
/// the review happened.
#[derive(Debug)]
pub struct ReviewedOrder {
    account_number: AccountNumber,
    origin: String,
    order: Order,
}

impl ReviewedOrder {
    /// The account this order was reviewed against.
    pub fn account_number(&self) -> &AccountNumber {
        &self.account_number
    }

    /// The order that was reviewed.
    pub fn order(&self) -> &Order {
        &self.order
    }
}

pub struct Account<'t> {
    pub(crate) inner: AccountInner,
    pub(crate) tasty: &'t TastyTrade,
}

impl Account<'_> {
    pub fn number(&self) -> AccountNumber {
        self.inner.account.account_number.clone()
    }

    pub async fn balance(&self) -> TastyResult<Balance> {
        let resp = self
            .tasty
            .get(&format!(
                "/accounts/{}/balances",
                self.inner.account.account_number.0
            ))
            .await?;
        Ok(resp)
    }

    pub async fn balance_snapshot(
        &self,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
        tod: SnapshotTimeOfDay,
        page_offset: usize,
    ) -> TastyResult<Paginated<BalanceSnapshot>> {
        let resp: Paginated<BalanceSnapshot> = self
            .tasty
            .get_with_query::<Items<BalanceSnapshot>, _, _>(
                &format!(
                    "/accounts/{}/balance-snapshots",
                    self.inner.account.account_number.0
                ),
                &[
                    ("start-date", &start_date.format("%Y-%m-%d").to_string()),
                    ("end-date", &end_date.format("%Y-%m-%d").to_string()),
                    ("page-offset", &page_offset.to_string()),
                    ("time-of-day", &tod.to_string()),
                ],
            )
            .await?;
        Ok(resp)
    }

    pub async fn positions(&self) -> TastyResult<Vec<FullPosition>> {
        let resp: Items<FullPosition> = self
            .tasty
            .get(&format!(
                "/accounts/{}/positions",
                self.inner.account.account_number.0
            ))
            .await?;
        resp.into_items()
    }

    pub async fn live_orders(&self) -> TastyResult<Vec<LiveOrderRecord>> {
        let resp: Items<LiveOrderRecord> = self
            .tasty
            .get(&format!(
                "/accounts/{}/orders/live",
                self.inner.account.account_number.0
            ))
            .await?;
        resp.into_items()
    }

    /// Dry-runs `order` and returns evidence bound to this account and this
    /// exact order.
    ///
    /// This is the entry point to the reviewed-placement flow. The receipt
    /// cannot be constructed any other way, so a `ReviewedOrder` is proof that
    /// the venue was asked about *this* order against *this* account, and that
    /// whoever holds it had the chance to read the answer.
    pub async fn review_order(&self, order: &Order) -> TastyResult<DryRunReceipt> {
        let result = self.dry_run(order).await?;

        Ok(DryRunReceipt {
            account_number: self.number(),
            origin: self.tasty.config.base_url.clone(),
            order: order.clone(),
            result,
        })
    }

    /// Places an order that came through [`Account::review_order`].
    ///
    /// # Errors
    ///
    /// Returns [`TastyTradeError::ConfigError`] when the receipt belongs to a
    /// different account. A receipt is bound to the account it was reviewed
    /// against, and buying power, permissions and positions are all per
    /// account, so a review against one says nothing about another.
    pub async fn place_reviewed_order(
        &self,
        reviewed: ReviewedOrder,
    ) -> TastyResult<OrderPlacedResult> {
        if reviewed.account_number != self.number() {
            return Err(crate::TastyTradeError::Precondition(
                "this order was reviewed against a different account; \
                 review it again against the account you mean to trade"
                    .to_string(),
            ));
        }

        // An account number is text and certification reuses production
        // numbering, so without this a sandbox dry run would authorise a real
        // order against the same number.
        if reviewed.origin != self.tasty.config.base_url {
            return Err(crate::TastyTradeError::Precondition(
                "this order was reviewed against a different venue; \
                 a dry run on one environment says nothing about another"
                    .to_string(),
            ));
        }

        self.place_order(&reviewed.order).await
    }

    /// Dry-runs an order without producing a receipt.
    ///
    /// Useful for pricing and what-if questions. For actually placing
    /// something, [`Account::review_order`] carries the answer forward.
    pub async fn dry_run(&self, order: &Order) -> TastyResult<DryRunResult> {
        let resp: DryRunResult = self
            .tasty
            .post(
                &format!(
                    "/accounts/{}/orders/dry-run",
                    self.inner.account.account_number.0
                ),
                order,
            )
            .await?;
        Ok(resp)
    }

    /// Places an order directly, with no evidence it was ever dry-run.
    ///
    /// Prefer [`Account::review_order`] followed by
    /// [`Account::place_reviewed_order`]: that path makes the venue's warnings
    /// impossible to skip past without saying so. This one remains for callers
    /// that manage the review themselves.
    pub async fn place_order(&self, order: &Order) -> TastyResult<OrderPlacedResult> {
        let resp: OrderPlacedResult = self
            .tasty
            .post(
                &format!("/accounts/{}/orders", self.inner.account.account_number.0),
                order,
            )
            .await?;
        Ok(resp)
    }

    pub async fn cancel_order(&self, id: OrderId) -> TastyResult<LiveOrderRecord> {
        self.tasty
            .delete(&format!(
                "/accounts/{}/orders/{}",
                self.inner.account.account_number.0, id.0
            ))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly the keys `GET /customers/me/accounts` returns against
    /// `api.cert.tastyworks.com`: no `is-test-drive`, no `external-id`, no
    /// `funding-date`, plus five keys the struct used to ignore.
    const CERT_ACCOUNT: &str = r#"{
        "account-number": "5WX12345",
        "account-type-name": "Individual",
        "created-at": "2025-01-14T10:22:41.000+00:00",
        "day-trader-status": false,
        "investment-objective": "SPECULATION",
        "is-closed": false,
        "is-firm-error": false,
        "is-firm-proprietary": false,
        "is-foreign": false,
        "is-futures-approved": true,
        "margin-or-cash": "Margin",
        "nickname": "Individual",
        "opened-at": "2025-01-14T10:22:41.000+00:00",
        "suitable-options-level": "Defined Risk Spreads"
    }"#;

    /// The production shape: `is-test-drive` present, none of the keys that
    /// only certification was observed to send.
    const PRODUCTION_ACCOUNT: &str = r#"{
        "account-number": "5WX54321",
        "external-id": "A1b2C3",
        "opened-at": "2024-03-02T09:00:00.000+00:00",
        "nickname": "Main",
        "account-type-name": "Individual",
        "day-trader-status": false,
        "is-firm-error": false,
        "is-firm-proprietary": false,
        "is-test-drive": false,
        "margin-or-cash": "Margin",
        "is-foreign": false,
        "funding-date": "2024-03-05"
    }"#;

    #[test]
    fn parses_the_certification_payload() {
        let account: AccountDetails =
            serde_json::from_str(CERT_ACCOUNT).expect("certification accounts must parse");

        assert_eq!(account.account_number.0, "5WX12345");
        // Absent in certification, defaulted rather than fatal.
        assert!(!account.is_test_drive);
        assert_eq!(account.external_id, None);
        assert_eq!(account.funding_date, None);
        // Sent by certification, so reported as the venue stated it.
        assert_eq!(account.day_trader_status, Some(false));
        assert_eq!(account.is_firm_error, Some(false));
        // Present in certification, previously discarded.
        assert_eq!(account.is_closed, Some(false));
        assert_eq!(account.is_futures_approved, Some(true));
        assert_eq!(
            account.suitable_options_level.as_deref(),
            Some("Defined Risk Spreads")
        );
        assert_eq!(
            account.created_at.map(|t| t.to_rfc3339()),
            Some("2025-01-14T10:22:41+00:00".to_string()),
            "the timestamp must be parsed, not carried as text"
        );
    }

    /// A flag the venue did not send is unknown, not false. Reporting `false`
    /// for an omitted firm-error or day-trader signal would let a caller act on
    /// an answer the broker never gave.
    #[test]
    fn an_omitted_flag_is_unknown_rather_than_false() {
        const WITHOUT_FLAGS: &str = r#"{
            "account-number": "5WX12345",
            "account-type-name": "Individual",
            "margin-or-cash": "Margin",
            "nickname": "Individual",
            "opened-at": "2025-01-14T10:22:41.000+00:00"
        }"#;

        let account: AccountDetails =
            serde_json::from_str(WITHOUT_FLAGS).expect("missing flags must not be fatal");

        assert_eq!(account.is_firm_error, None);
        assert_eq!(account.is_firm_proprietary, None);
        assert_eq!(account.day_trader_status, None);
        assert_eq!(account.is_foreign, None);
    }

    #[test]
    fn parses_the_production_payload() {
        let account: AccountDetails =
            serde_json::from_str(PRODUCTION_ACCOUNT).expect("production accounts must parse");

        assert_eq!(account.account_number.0, "5WX54321");
        assert!(!account.is_test_drive);
        assert_eq!(account.external_id.as_deref(), Some("A1b2C3"));
        assert_eq!(account.is_firm_error, Some(false));
        // Not sent by production, so absent rather than wrong.
        assert_eq!(account.is_closed, None);
        assert_eq!(account.investment_objective, None);
        assert_eq!(account.created_at, None);
    }

    /// The bug as the caller experienced it: `Items<T>` skips what it cannot
    /// parse, so one strict field turned a live sandbox account into an empty
    /// list and `TastyTrade::account` reported the account as not on the
    /// session.
    #[test]
    fn certification_accounts_survive_the_items_envelope() {
        let body =
            format!(r#"{{"items":[{{"account":{CERT_ACCOUNT},"authority-level":"owner"}}]}}"#);

        let items: Items<AccountInner> =
            serde_json::from_str(&body).expect("the envelope is well formed");

        assert_eq!(items.items.len(), 1, "the sandbox account must survive");
        assert_eq!(items.items[0].account.account_number.0, "5WX12345");
        assert_eq!(items.items[0].authority_level, "owner");
    }
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    #[test]
    fn a_redacted_number_identifies_without_revealing() {
        let account = AccountNumber::from("5WX123456");
        let redacted = account.redacted();

        assert_eq!(redacted, "5W…456");
        assert!(
            !redacted.contains("X1234"),
            "the middle must not survive: {redacted}"
        );
    }

    /// Two accounts must still be distinguishable in a support thread.
    #[test]
    fn different_accounts_redact_differently() {
        assert_ne!(
            AccountNumber::from("5WX123456").redacted(),
            AccountNumber::from("5WX123789").redacted()
        );
    }

    /// A number short enough that a prefix and a suffix would reveal most of
    /// it is not partially redacted, it is hidden.
    #[test]
    fn a_short_number_is_masked_entirely() {
        for short in ["", "1", "12345"] {
            let redacted = AccountNumber::from(short).redacted();
            assert!(
                redacted.chars().all(|c| c == '*'),
                "{short:?} should be fully masked, got {redacted}"
            );
        }
    }
}
