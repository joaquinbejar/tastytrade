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

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountDetails {
    pub account_number: AccountNumber,
    pub external_id: Option<String>,
    pub opened_at: String,
    pub nickname: String,
    pub account_type_name: String,
    pub day_trader_status: bool,
    pub is_firm_error: bool,
    pub is_firm_proprietary: bool,
    pub is_test_drive: bool,
    pub margin_or_cash: String,
    pub is_foreign: bool,
    pub funding_date: Option<String>,
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
