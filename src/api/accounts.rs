use super::base::{Items, Paginated};
use crate::api::base::TastyResult;
use crate::api::query::{PageRequest, QueryBuilder};
use crate::api::url::encode_path_segment;
use crate::types::account_filter::{BalanceSnapshotFilter, PositionFilter, SnapshotRange};
use crate::types::balance::{Balance, BalanceSnapshot, SnapshotTimeOfDay};
use crate::types::margin::{
    EffectiveMarginRequirement, MarginEstimate, MarginOrderRequest, MarginRequirementsReport,
    PositionLimit,
};
use crate::types::net_liq::{NetLiqHistoryFilter, NetLiqOhlc};
use crate::types::order::{DryRunResult, Order, OrderId, OrderPlacedResult, Warning};
use crate::types::trading_status::TradingStatus;
use crate::types::transaction::{TotalFees, Transaction, TransactionFilter};
use crate::{FullPosition, LiveOrderRecord, TastyTrade};
use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};

#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone,
)]
#[serde(transparent)]
/// A broker-assigned account identifier.
///
/// Account PII: keep it out of logs and errors. [`AccountNumber::redacted`]
/// gives a form that is safe to write down.
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
/// An account as the listing endpoint returns it.
pub struct AccountInner {
    /// The account itself.
    pub account: AccountDetails,
    /// What this session may do with it, e.g. `owner`.
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
    /// Returns [`crate::TastyTradeError::Precondition`] when the venue attached
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

/// An account bound to the session that found it.
///
/// The lifetime ties it to its client, so an account cannot outlive the
/// session that can act on it.
pub struct Account<'t> {
    pub(crate) inner: AccountInner,
    pub(crate) tasty: &'t TastyTrade,
}

impl Account<'_> {
    /// This account's number.
    ///
    /// Account PII. Use [`AccountNumber::redacted`] before logging it.
    pub fn number(&self) -> AccountNumber {
        self.inner.account.account_number.clone()
    }

    /// Everything the venue said about this account.
    ///
    /// Nickname, type, margin-or-cash, the approval flags and the dates. A
    /// flag the broker did not send is `None`, never `false`.
    ///
    /// Account PII: [`AccountDetails::account_number`] is in here, so the same
    /// care applies as to [`Account::number`].
    pub fn details(&self) -> &AccountDetails {
        &self.inner.account
    }

    /// What this session may do with the account, e.g. `owner`.
    ///
    /// `None` when the account came from the single-account endpoint, which
    /// answers with the account itself rather than the listing's authority
    /// decorator, so there is no level to report. Not reported is not the same
    /// as none, and returning `""` made the two indistinguishable — the caller
    /// would have had to know which call produced the account to read the
    /// value correctly.
    ///
    /// The listing always sends the field, so an empty string can only come
    /// from the endpoint that does not send it at all.
    pub fn authority_level(&self) -> Option<&str> {
        Some(self.inner.authority_level.as_str()).filter(|level| !level.is_empty())
    }

    /// `/accounts/{this account}{suffix}`, with the number percent-encoded.
    ///
    /// Every account-scoped request builds its path here, so no endpoint can
    /// be added that forgets the encoding — the seven that existed before this
    /// each interpolated the number raw. `suffix` starts with `/` and carries
    /// any further dynamic segment already encoded, because only its caller
    /// knows where the boundaries between segments are.
    fn path(&self, suffix: &str) -> String {
        format!(
            "/accounts/{}{suffix}",
            encode_path_segment(&self.inner.account.account_number.0)
        )
    }

    /// Every current balance row, one per currency the account holds.
    ///
    /// Every monetary field is `Decimal`.
    ///
    /// # Errors
    ///
    /// Fails when balances arrive but none can be decoded, which is a defect
    /// in this crate rather than an account with no money. Propagates the
    /// venue's error otherwise; the response body never reaches it.
    pub async fn balances(&self) -> TastyResult<Vec<Balance>> {
        let resp: Items<Balance> = self.tasty.get(&self.path("/balances")).await?;
        resp.into_items()
    }

    /// The account's single balance row.
    ///
    /// The endpoint answers with an `items` envelope — it has since the venue
    /// changed it on 2024-05-01 — so "the balance" only means something when
    /// exactly one row came back. This decodes the envelope properly and says
    /// so when it does not, rather than picking a currency for the caller.
    ///
    /// # Errors
    ///
    /// [`crate::TastyTradeError::Precondition`] when the venue returned any
    /// number of rows other than one: the request succeeded and the answer
    /// does not fit the question, so retrying changes nothing. Use
    /// [`Account::balances`] or [`Account::balance_in`] instead. Otherwise as
    /// [`Account::balances`].
    pub async fn balance(&self) -> TastyResult<Balance> {
        let mut rows = self.balances().await?;

        if rows.len() == 1 {
            // `swap_remove` rather than indexing: the length is known and this
            // moves the row out without a clone or an unwrap.
            return Ok(rows.swap_remove(0));
        }

        // Currency codes are schema; the amounts beside them are not, and an
        // error travels wherever the caller sends it.
        let currencies: Vec<&str> = rows
            .iter()
            .map(|row| row.currency.as_deref().unwrap_or("unnamed"))
            .collect();

        Err(crate::TastyTradeError::Precondition(format!(
            "the account returned {} balance row(s) ({}), so there is no single \
             balance to return; use balances() for all of them or \
             balance_in(currency) for one",
            rows.len(),
            currencies.join(", ")
        )))
    }

    /// The balance row for one currency.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404` for a currency the
    /// account does not hold.
    pub async fn balance_in(&self, currency: &str) -> TastyResult<Balance> {
        self.tasty
            .get(&self.path(&format!("/balances/{}", encode_path_segment(currency))))
            .await
    }

    /// Historical balance snapshots.
    ///
    /// `filter` carries the whole documented query: the time of day (which the
    /// venue requires), a single day **or** a date range, a currency and a
    /// page.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, and fails if the endpoint answers
    /// without a pagination block.
    pub async fn balance_snapshots(
        &self,
        filter: &BalanceSnapshotFilter,
    ) -> TastyResult<Paginated<BalanceSnapshot>> {
        let query = filter.to_query();
        self.tasty
            .get_with_query::<Items<BalanceSnapshot>, _, _>(
                &self.path("/balance-snapshots"),
                &query.pairs(),
            )
            .await
    }

    /// Historical balance snapshots, by positional argument.
    ///
    /// The 0.3 signature, forwarding to [`Account::balance_snapshots`]. It
    /// could reach only one of the two date shapes the venue documents and had
    /// no way to send a currency, which is why the filter replaced it — but
    /// removing it outright would break every existing caller for no reason
    /// the caller can act on at the call site.
    ///
    /// # Errors
    ///
    /// As [`Account::balance_snapshots`].
    #[deprecated(
        since = "0.4.0",
        note = "use `balance_snapshots(&BalanceSnapshotFilter)`, which reaches the whole \
                documented query rather than four of its parameters"
    )]
    pub async fn balance_snapshot(
        &self,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
        tod: SnapshotTimeOfDay,
        page_offset: usize,
    ) -> TastyResult<Paginated<BalanceSnapshot>> {
        let page_offset = u32::try_from(page_offset).map_err(|_| {
            crate::TastyTradeError::Precondition(format!(
                "page offset {page_offset} does not fit the u32 the venue accepts"
            ))
        })?;

        self.balance_snapshots(
            &BalanceSnapshotFilter::at(tod)
                .with_range(SnapshotRange::Range {
                    start: Some(start_date),
                    end: Some(end_date),
                })
                .with_page(PageRequest::new().with_page_offset(page_offset)),
        )
        .await
    }

    /// Open positions.
    ///
    /// # Errors
    ///
    /// Fails when positions arrive but none can be decoded, which is a defect
    /// in this crate rather than a flat account. A genuinely empty list is
    /// `Ok`.
    pub async fn positions(&self) -> TastyResult<Vec<FullPosition>> {
        self.positions_matching(&PositionFilter::new()).await
    }

    /// Positions the venue selects, rather than every open one.
    ///
    /// The filters are applied at the venue: asking for one underlying
    /// downloads one underlying. An empty [`PositionFilter`] sends no query
    /// parameters at all, so it is byte for byte the request
    /// [`Account::positions`] makes.
    ///
    /// # Errors
    ///
    /// As [`Account::positions`].
    pub async fn positions_matching(
        &self,
        filter: &PositionFilter,
    ) -> TastyResult<Vec<FullPosition>> {
        let query = filter.to_query();
        let resp: Items<FullPosition> = self
            .tasty
            .get_with_query(&self.path("/positions"), &query.pairs())
            .await?;
        resp.into_items()
    }

    /// One page of the account's ledger.
    ///
    /// Everything that changed a balance or a position: fills, fees,
    /// dividends, assignments, cash movements. `filter` carries the whole
    /// documented query.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint answers without a pagination block, and when
    /// transactions arrive but none can be decoded. A genuinely empty page is
    /// `Ok`.
    pub async fn transactions(
        &self,
        filter: &TransactionFilter,
    ) -> TastyResult<Paginated<Transaction>> {
        let query = filter.to_query();
        self.tasty
            .get_with_query::<Items<Transaction>, _, _>(&self.path("/transactions"), &query.pairs())
            .await
    }

    /// One transaction by its identifier.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404` for an identifier this
    /// account does not have.
    pub async fn transaction(&self, id: i64) -> TastyResult<Transaction> {
        // `id` is an `i64`, so its rendering is already path-safe. The type is
        // what guarantees that, not this call site.
        self.tasty
            .get(&self.path(&format!("/transactions/{id}")))
            .await
    }

    /// What the account paid in fees on one day.
    ///
    /// `None` omits the `date` parameter, which leaves the venue's documented
    /// default of today in place — sending today's date from this process
    /// would substitute *this machine's* idea of the date for the venue's.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn total_fees(&self, date: Option<NaiveDate>) -> TastyResult<TotalFees> {
        let mut query = QueryBuilder::new();
        query.push_opt("date", date);

        self.tasty
            .get_with_query::<TotalFees, TotalFees, _>(
                &self.path("/transactions/total-fees"),
                &query.pairs(),
            )
            .await
    }

    /// Whether the account may trade, and what it may trade.
    ///
    /// The cheap check before an order: a closed or frozen account cannot trade
    /// at all, a closing-only account can only reduce, and the feature flags
    /// decide whether futures, cryptocurrency or uncovered short calls are
    /// available. It also carries the live day-trade count.
    ///
    /// Every flag is `Option<bool>`: one the venue omitted is unknown, never
    /// `false`.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn trading_status(&self) -> TastyResult<TradingStatus> {
        self.tasty.get(&self.path("/trading-status")).await
    }

    /// The account's current margin and capital requirements, by underlying.
    ///
    /// The standing requirement, as opposed to the effect of one order. Nested
    /// three levels — total, per underlying, per margin strategy — because the
    /// per-strategy figures are what explain the total.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn margin_requirements(&self) -> TastyResult<MarginRequirementsReport> {
        self.tasty
            .get(&format!(
                "/margin/accounts/{}/requirements",
                encode_path_segment(&self.inner.account.account_number.0)
            ))
            .await
    }

    /// Estimates the margin one order would consume.
    ///
    /// **Routes nothing.** Named to keep it apart from
    /// [`Account::dry_run`], which is the order preflight against
    /// `/accounts/{n}/orders/dry-run`: this one answers "how much buying power
    /// would that take", that one answers "would the venue accept it". There is
    /// no path from here to a placement.
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the request names a
    /// different account, has a blank underlying or symbol, carries no legs or
    /// more than [`crate::prelude::MAX_MARGIN_LEGS`], or repeats a leg.
    /// Propagates the venue's error otherwise.
    pub async fn estimate_margin(
        &self,
        request: &MarginOrderRequest,
    ) -> TastyResult<MarginEstimate> {
        request.validate(&self.inner.account.account_number.0)?;

        self.tasty
            .post(
                &format!(
                    "/margin/accounts/{}/dry-run",
                    encode_path_segment(&self.inner.account.account_number.0)
                ),
                request,
            )
            .await
    }

    /// The standing margin requirement for one underlying.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn effective_margin_requirement(
        &self,
        underlying_symbol: &str,
    ) -> TastyResult<EffectiveMarginRequirement> {
        self.tasty
            .get(&self.path(&format!(
                "/margin-requirements/{}/effective",
                encode_path_segment(underlying_symbol)
            )))
            .await
    }

    /// How much of each instrument type this account may order and hold.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn position_limit(&self) -> TastyResult<PositionLimit> {
        self.tasty.get(&self.path("/position-limit")).await
    }

    /// The account's equity curve.
    ///
    /// Open, high, low and close of net liquidating value over time — what a
    /// performance or drawdown chart is drawn from.
    ///
    /// **Live only.** The venue's sandbox page lists Net Liq History as
    /// unavailable in certification, so this returns nothing useful there.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which
    /// is a defect in this crate's model rather than an account with no
    /// history. Propagates the venue's error otherwise.
    pub async fn net_liq_history(
        &self,
        filter: &NetLiqHistoryFilter,
    ) -> TastyResult<Vec<NetLiqOhlc>> {
        let query = filter.to_query();
        let resp: Items<NetLiqOhlc> = self
            .tasty
            .get_with_query(&self.path("/net-liq/history"), &query.pairs())
            .await?;
        resp.into_items()
    }

    /// Orders that are still working.
    ///
    /// # Errors
    ///
    /// As [`Account::positions`].
    pub async fn live_orders(&self) -> TastyResult<Vec<LiveOrderRecord>> {
        let resp: Items<LiveOrderRecord> = self.tasty.get(&self.path("/orders/live")).await?;
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
    /// Returns [`crate::TastyTradeError::Precondition`] when the receipt belongs to a
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
            .post(&self.path("/orders/dry-run"), order)
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
        let resp: OrderPlacedResult = self.tasty.post(&self.path("/orders"), order).await?;
        Ok(resp)
    }

    /// Cancels a working order.
    ///
    /// **Mutates account state.** Cancelling an order that has already filled
    /// is the venue's decision to refuse, not this crate's.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a refusal to cancel.
    pub async fn cancel_order(&self, id: OrderId) -> TastyResult<LiveOrderRecord> {
        self.tasty
            // `OrderId` is a `u64`, so its decimal rendering is already inside
            // the unreserved set and encoding it would be a no-op. The type is
            // what guarantees that, not this call site.
            .delete(&self.path(&format!("/orders/{}", id.0)))
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
