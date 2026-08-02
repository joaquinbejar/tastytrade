use super::base::{Items, Paginated};
use crate::api::base::TastyResult;
use crate::types::balance::{Balance, BalanceSnapshot, SnapshotTimeOfDay};
use crate::types::order::{DryRunResult, Order, OrderId, OrderPlacedResult};
use crate::{FullPosition, LiveOrderRecord, TastyTrade};
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

/// Details of a single trading account.
///
/// The certification environment and production do not return the same set of
/// keys, and either side gains fields over time. Every flag therefore defaults
/// to `false` when absent rather than failing the whole account: a missing
/// boolean is never worth losing an account lookup over, and `Items<T>` skips
/// items it cannot parse, so a strict field turns a live account into an empty
/// list rather than an error.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountDetails {
    /// Broker-assigned account identifier.
    pub account_number: AccountNumber,
    /// External identifier, when the account carries one.
    pub external_id: Option<String>,
    /// Timestamp the account was opened, RFC 3339.
    pub opened_at: String,
    /// User-facing name of the account.
    pub nickname: String,
    /// Account type as named by the broker, e.g. `Individual`.
    pub account_type_name: String,
    /// Whether the account is flagged as a pattern day trader.
    #[serde(default)]
    pub day_trader_status: bool,
    /// Whether the account is in a firm error state.
    #[serde(default)]
    pub is_firm_error: bool,
    /// Whether the account is firm proprietary.
    #[serde(default)]
    pub is_firm_proprietary: bool,
    /// Whether the account is a test-drive account. Absent in certification.
    #[serde(default)]
    pub is_test_drive: bool,
    /// Whether the account is margin or cash.
    pub margin_or_cash: String,
    /// Whether the account is foreign.
    #[serde(default)]
    pub is_foreign: bool,
    /// Date the account was funded, when it has been.
    pub funding_date: Option<String>,
    /// Whether the account has been closed. `None` when the venue omits it.
    pub is_closed: Option<bool>,
    /// Timestamp the account record was created, RFC 3339.
    pub created_at: Option<String>,
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
        Ok(resp.items)
    }

    pub async fn live_orders(&self) -> TastyResult<Vec<LiveOrderRecord>> {
        let resp: Items<LiveOrderRecord> = self
            .tasty
            .get(&format!(
                "/accounts/{}/orders/live",
                self.inner.account.account_number.0
            ))
            .await?;
        Ok(resp.items)
    }

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
        // Present in certification, previously discarded.
        assert_eq!(account.is_closed, Some(false));
        assert_eq!(account.is_futures_approved, Some(true));
        assert_eq!(
            account.suitable_options_level.as_deref(),
            Some("Defined Risk Spreads")
        );
    }

    #[test]
    fn parses_the_production_payload() {
        let account: AccountDetails =
            serde_json::from_str(PRODUCTION_ACCOUNT).expect("production accounts must parse");

        assert_eq!(account.account_number.0, "5WX54321");
        assert!(!account.is_test_drive);
        assert_eq!(account.external_id.as_deref(), Some("A1b2C3"));
        // Not sent by production, so absent rather than wrong.
        assert_eq!(account.is_closed, None);
        assert_eq!(account.investment_objective, None);
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
