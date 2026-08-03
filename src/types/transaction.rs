//! The account's ledger: fills, fees, dividends, assignments, cash movements.
//!
//! A transaction is any event that changed a balance or a position. This is the
//! only place a P&L can be reconciled from — an order tells you what was asked
//! for, a transaction tells you what happened and what it cost.
//!
//! Every field except the identifier is `Option<T>`. The venue sends what
//! applies: a dividend has no commission, a cash transfer has no symbol, and a
//! fee row has no quantity. A field the venue omitted is unknown, never zero —
//! a commission that defaults to zero is a P&L that is quietly wrong.

use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::api::query::{PageRequest, QueryBuilder};
use crate::types::instrument::InstrumentType;
use crate::types::order::PriceEffect;
use crate::types::wire::wire_enum;

wire_enum! {
    /// What kind of event a transaction records.
    ///
    /// The four values the venue documents for the `type` and `types` filters.
    /// The `Unknown` arm is the design, not a concession: `Items<T>` drops what
    /// it cannot parse, so a strict enum would make a new transaction kind
    /// disappear from a ledger without an error.
    TransactionType {
        AdministrativeTransfer => "Administrative Transfer",
        MoneyMovement => "Money Movement",
        ReceiveDeliver => "Receive Deliver",
        Trade => "Trade",
    }
}

wire_enum! {
    /// The specific event within a [`TransactionType`].
    ///
    /// The twenty-five values the venue documents for the `sub-type` filter.
    TransactionSubType {
        Acat => "ACAT",
        Assignment => "Assignment",
        BalanceAdjustment => "Balance Adjustment",
        CashMerger => "Cash Merger",
        CashSettledAssignment => "Cash Settled Assignment",
        CashSettledExercise => "Cash Settled Exercise",
        CreditInterest => "Credit Interest",
        DebitInterest => "Debit Interest",
        Deposit => "Deposit",
        Dividend => "Dividend",
        Exercise => "Exercise",
        Expiration => "Expiration",
        Fee => "Fee",
        ForwardSplit => "Forward Split",
        FullyPaidStockLendingIncome => "Fully Paid Stock Lending Income",
        FuturesSettlement => "Futures Settlement",
        MarkToMarket => "Mark to Market",
        Maturity => "Maturity",
        ReverseSplit => "Reverse Split",
        ReverseSplitRemoval => "Reverse Split Removal",
        SpecialDividend => "Special Dividend",
        StockMerger => "Stock Merger",
        StockMergerRemoval => "Stock Merger Removal",
        SymbolChange => "Symbol Change",
        Transfer => "Transfer",
        Withdrawal => "Withdrawal",
    }
}

wire_enum! {
    /// What a transaction did to a position.
    ///
    /// Deliberately **not** [`crate::types::order::Action`], which is the
    /// order-placement enum. That one is strict on purpose: an order with an
    /// action this crate does not recognise is an order nobody should be able
    /// to build. This is the read side, where tolerance is what keeps a ledger
    /// complete — the venue's own swagger lists `Allocate` here and its guide
    /// does not.
    TransactionAction {
        Allocate => "Allocate",
        Buy => "Buy",
        BuyToClose => "Buy to Close",
        BuyToOpen => "Buy to Open",
        Sell => "Sell",
        SellToClose => "Sell to Close",
        SellToOpen => "Sell to Open",
    }
}

/// One row of an account's ledger.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Transaction {
    /// The venue's identifier for this transaction.
    pub id: i64,
    /// Which account it belongs to. Account PII.
    #[serde(default)]
    pub account_number: Option<String>,
    /// What kind of event this was.
    #[serde(default)]
    pub transaction_type: Option<TransactionType>,
    /// The specific event within that kind.
    #[serde(default)]
    pub transaction_sub_type: Option<TransactionSubType>,
    /// What it did to a position, when it touched one.
    #[serde(default)]
    pub action: Option<TransactionAction>,
    /// The venue's prose description.
    ///
    /// Venue data written for a person, like an order `Warning`:
    /// it can name an amount or an instrument, so it goes in front of somebody
    /// rather than into `tracing`.
    #[serde(default)]
    pub description: Option<String>,
    /// The instrument, when there is one.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The underlying, for derivatives.
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// What kind of instrument it was.
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
    /// How many units.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub quantity: Option<Decimal>,
    /// Price per unit.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub price: Option<Decimal>,
    /// The gross value.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub value: Option<Decimal>,
    /// Whether the gross value was a credit or a debit.
    #[serde(default)]
    pub value_effect: Option<PriceEffect>,
    /// The value after fees and commission — what actually moved.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub net_value: Option<Decimal>,
    /// Whether the net value was a credit or a debit.
    #[serde(default)]
    pub net_value_effect: Option<PriceEffect>,
    /// Commission charged.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub commission: Option<Decimal>,
    /// Whether the commission was a credit or a debit.
    #[serde(default)]
    pub commission_effect: Option<PriceEffect>,
    /// Clearing fees.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub clearing_fees: Option<Decimal>,
    /// Whether the clearing fees were a credit or a debit.
    #[serde(default)]
    pub clearing_fees_effect: Option<PriceEffect>,
    /// Regulatory fees.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub regulatory_fees: Option<Decimal>,
    /// Whether the regulatory fees were a credit or a debit.
    #[serde(default)]
    pub regulatory_fees_effect: Option<PriceEffect>,
    /// Proprietary index option fees.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub proprietary_index_option_fees: Option<Decimal>,
    /// Whether those fees were a credit or a debit.
    #[serde(default)]
    pub proprietary_index_option_fees_effect: Option<PriceEffect>,
    /// Currency conversion fees.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub currency_conversion_fees: Option<Decimal>,
    /// Whether those fees were a credit or a debit.
    #[serde(default)]
    pub currency_conversion_fees_effect: Option<PriceEffect>,
    /// Any other charge.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub other_charge: Option<Decimal>,
    /// What that other charge was for.
    #[serde(default)]
    pub other_charge_description: Option<String>,
    /// Whether the other charge was a credit or a debit.
    #[serde(default)]
    pub other_charge_effect: Option<PriceEffect>,
    /// Whether the fees on this row are an estimate rather than settled.
    ///
    /// `None` means the venue did not say, which is not the same as "settled".
    #[serde(default)]
    pub is_estimated_fee: Option<bool>,
    /// The agency price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub agency_price: Option<Decimal>,
    /// The principal price.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub principal_price: Option<Decimal>,
    /// Which currency this row is denominated in.
    #[serde(default)]
    pub currency: Option<String>,
    /// Where the order was routed.
    #[serde(default)]
    pub destination_venue: Option<String>,
    /// Which exchange executed it.
    #[serde(default)]
    pub exchange: Option<String>,
    /// The exchange affiliation identifier.
    #[serde(default)]
    pub exchange_affiliation_identifier: Option<String>,
    /// The execution identifier.
    #[serde(default)]
    pub exec_id: Option<String>,
    /// The exchange's own execution identifier.
    #[serde(default)]
    pub ext_exec_id: Option<String>,
    /// The exchange's order number.
    #[serde(default)]
    pub ext_exchange_order_number: Option<String>,
    /// The global order number.
    #[serde(default)]
    pub ext_global_order_number: Option<i64>,
    /// The group fill identifier.
    #[serde(default)]
    pub ext_group_fill_id: Option<String>,
    /// The group identifier.
    #[serde(default)]
    pub ext_group_id: Option<String>,
    /// How many legs the originating order had.
    #[serde(default)]
    pub leg_count: Option<i64>,
    /// The order that produced this transaction.
    #[serde(default)]
    pub order_id: Option<i64>,
    /// The transaction this one reverses, for a correction.
    #[serde(default)]
    pub reverses_id: Option<i64>,
    /// Tax lots consumed or created.
    ///
    /// `Value` rather than a modelled type: the venue's schema types this as an
    /// object with **no properties at all**, so there is nothing to model
    /// against. Anything decodes, which is what keeps a lotted transaction from
    /// being dropped by `Items<T>` over a field nobody has documented.
    #[serde(default)]
    pub lots: Option<Value>,
    /// The trading day this belongs to.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub transaction_date: Option<NaiveDate>,
    /// The cost-basis reconciliation date.
    #[serde(default, with = "crate::types::wire::date_option")]
    pub cost_basis_reconciliation_date: Option<NaiveDate>,
    /// When it executed, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub executed_at: Option<DateTime<FixedOffset>>,
    /// When the record was created, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub created_at: Option<DateTime<FixedOffset>>,
}

/// What an account paid in fees on one day.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TotalFees {
    /// The total, as a magnitude. Read
    /// [`TotalFees::total_fees_effect`] for the direction.
    #[serde(default, with = "crate::types::wire::decimal_option")]
    pub total_fees: Option<Decimal>,
    /// Whether the total was a debit or a credit.
    ///
    /// The amount is unsigned, so this is not decoration: a `Debit` of 100 and
    /// a `Credit` of 100 are opposite facts about the same number.
    #[serde(default)]
    pub total_fees_effect: Option<PriceEffect>,
}

/// Which transaction kinds to select.
///
/// The venue documents `type` and `types` as **mutually exclusive** — "you can
/// only include one or the other" — so they are one enum here and a request
/// carrying both cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionTypes {
    /// One kind, sent as `type`.
    One(TransactionType),
    /// Several, sent as repeated `types[]` keys.
    Several(Vec<TransactionType>),
}

/// Which direction to sort a transaction listing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionSort {
    /// Newest first, which is what the venue does when nothing is asked for.
    #[default]
    Descending,
    /// Oldest first.
    Ascending,
}

impl TransactionSort {
    /// The text the venue uses.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Descending => "Desc",
            Self::Ascending => "Asc",
        }
    }
}

/// Which transactions to fetch.
///
/// `Default` is the whole ledger, newest first, one venue-sized page at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionFilter {
    page: PageRequest,
    types: Option<TransactionTypes>,
    sub_types: Vec<TransactionSubType>,
    sort: Option<TransactionSort>,
    action: Option<TransactionAction>,
    instrument_type: Option<InstrumentType>,
    currency: Option<String>,
    symbol: Option<String>,
    underlying_symbol: Option<String>,
    futures_symbol: Option<String>,
    partition_key: Option<String>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    start_at: Option<DateTime<FixedOffset>>,
    end_at: Option<DateTime<FixedOffset>>,
}

impl TransactionFilter {
    /// The whole ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts to transaction kinds. One or several, never both.
    #[must_use]
    pub fn with_types(mut self, types: TransactionTypes) -> Self {
        self.types = Some(types);
        self
    }

    /// Restricts to sub-types, sent as repeated `sub-type[]` keys.
    #[must_use]
    pub fn with_sub_types(mut self, sub_types: &[TransactionSubType]) -> Self {
        self.sub_types.extend(sub_types.iter().cloned());
        self
    }

    /// Which order to sort in.
    #[must_use]
    pub fn with_sort(mut self, sort: TransactionSort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Restricts to one action.
    #[must_use]
    pub fn with_action(mut self, action: TransactionAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Restricts to one instrument type.
    #[must_use]
    pub fn with_instrument_type(mut self, instrument_type: InstrumentType) -> Self {
        self.instrument_type = Some(instrument_type);
        self
    }

    /// Restricts to one currency.
    #[must_use]
    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }

    /// Restricts to one symbol.
    #[must_use]
    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    /// Restricts to one underlying.
    #[must_use]
    pub fn with_underlying_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.underlying_symbol = Some(symbol.into());
        self
    }

    /// Restricts to one full futures symbol, e.g. `/ESU9`.
    #[must_use]
    pub fn with_futures_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.futures_symbol = Some(symbol.into());
        self
    }

    /// Restricts to one account partition.
    #[must_use]
    pub fn with_partition_key(mut self, key: impl Into<String>) -> Self {
        self.partition_key = Some(key.into());
        self
    }

    /// Restricts to a range of trading days.
    #[must_use]
    pub fn with_dates(mut self, start: Option<NaiveDate>, end: Option<NaiveDate>) -> Self {
        self.start_date = start;
        self.end_date = end;
        self
    }

    /// Restricts to a range of instants, offsets preserved.
    #[must_use]
    pub fn with_times(
        mut self,
        start: Option<DateTime<FixedOffset>>,
        end: Option<DateTime<FixedOffset>>,
    ) -> Self {
        self.start_at = start;
        self.end_at = end;
        self
    }

    /// Which page to ask for.
    #[must_use]
    pub fn with_page(mut self, page: PageRequest) -> Self {
        self.page = page;
        self
    }

    /// The page this filter asks for.
    pub fn page(&self) -> PageRequest {
        self.page
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        self.page.write_into(&mut query);
        query.push_opt("sort", self.sort.map(|sort| sort.as_wire()));

        match &self.types {
            Some(TransactionTypes::One(one)) => query.push("type", one.as_wire()),
            Some(TransactionTypes::Several(many)) => query.push_each(
                "types[]",
                many.iter().map(|kind| kind.as_wire().to_string()),
            ),
            None => {}
        }

        query.push_each(
            "sub-type[]",
            self.sub_types.iter().map(|sub| sub.as_wire().to_string()),
        );
        query.push_opt(
            "action",
            self.action.as_ref().map(TransactionAction::as_wire),
        );
        query.push_opt(
            "instrument-type",
            self.instrument_type.as_ref().map(ToString::to_string),
        );
        query.push_opt("currency", self.currency.as_ref());
        query.push_opt("symbol", self.symbol.as_ref());
        query.push_opt("underlying-symbol", self.underlying_symbol.as_ref());
        query.push_opt("futures-symbol", self.futures_symbol.as_ref());
        query.push_opt("partition-key", self.partition_key.as_ref());
        query.push_opt("start-date", self.start_date);
        query.push_opt("end-date", self.end_date);
        query.push_opt("start-at", self.start_at.map(|at| at.to_rfc3339()));
        query.push_opt("end-at", self.end_at.map(|at| at.to_rfc3339()));
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The payload from the venue's own guide, unedited apart from the account
    /// number.
    const LISTING: &str = include_str!("../../Doc/transactions_listing.json");

    #[test]
    fn a_dividend_transaction_decodes_from_the_venues_own_payload() {
        let body: serde_json::Value = serde_json::from_str(LISTING).expect("valid JSON");
        let items = body["data"]["items"]
            .as_array()
            .expect("the listing carries items");

        let first: Transaction =
            serde_json::from_value(items[0].clone()).expect("the row must decode");

        assert_eq!(first.id, 252640963);
        assert_eq!(
            first.transaction_type,
            Some(TransactionType::ReceiveDeliver)
        );
        assert_eq!(
            first.transaction_sub_type,
            Some(TransactionSubType::Dividend)
        );
        assert_eq!(first.action, Some(TransactionAction::BuyToOpen));
        // Money is Decimal, and the venue sends it quoted.
        assert_eq!(first.quantity.expect("a quantity").to_string(), "1.68074");
        assert_eq!(first.price.expect("a price").to_string(), "16.46");
        assert_eq!(first.value_effect, Some(PriceEffect::None));
        assert_eq!(first.is_estimated_fee, Some(true));
        // A dividend has no commission, and the venue omits it rather than
        // sending zero. So do we.
        assert_eq!(first.commission, None);
    }

    /// A cash row has no symbol-side fields at all, which is the case a struct
    /// of required fields would reject.
    #[test]
    fn a_money_movement_row_without_a_quantity_still_decodes() {
        let body: serde_json::Value = serde_json::from_str(LISTING).expect("valid JSON");
        let items = body["data"]["items"].as_array().expect("items");
        let cash: Transaction =
            serde_json::from_value(items[2].clone()).expect("the row must decode");

        assert_eq!(cash.transaction_type, Some(TransactionType::MoneyMovement));
        assert_eq!(cash.quantity, None);
        assert_eq!(cash.action, None);
        assert_eq!(cash.net_value_effect, Some(PriceEffect::Credit));
    }

    /// A kind the venue adds tomorrow keeps its text instead of making the row
    /// disappear through `Items<T>`.
    #[test]
    fn an_unrecognised_kind_survives_verbatim() {
        let row: Transaction = serde_json::from_str(
            r#"{"id": 1, "transaction-type": "Quantum Entanglement",
                "transaction-sub-type": "Spooky Action"}"#,
        )
        .expect("the row must still decode");

        assert_eq!(
            row.transaction_type,
            Some(TransactionType::Unknown("Quantum Entanglement".to_string()))
        );
        assert!(!row.transaction_sub_type.expect("a sub-type").is_known());
    }

    /// `type` and `types` are mutually exclusive at the venue, and the enum
    /// makes sending both impossible.
    #[test]
    fn one_kind_and_several_kinds_can_never_be_sent_together() {
        let one =
            TransactionFilter::new().with_types(TransactionTypes::One(TransactionType::Trade));
        assert_eq!(one.to_query().pairs(), vec![("type", "Trade")]);

        let several = TransactionFilter::new().with_types(TransactionTypes::Several(vec![
            TransactionType::Trade,
            TransactionType::MoneyMovement,
        ]));
        assert_eq!(
            several.to_query().pairs(),
            vec![("types[]", "Trade"), ("types[]", "Money Movement")]
        );
    }

    #[test]
    fn sub_types_are_repeated_keys_in_the_venues_spelling() {
        let filter = TransactionFilter::new().with_sub_types(&[
            TransactionSubType::Dividend,
            TransactionSubType::CashSettledAssignment,
        ]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("sub-type[]", "Dividend"),
                ("sub-type[]", "Cash Settled Assignment"),
            ]
        );
    }

    #[test]
    fn an_unfiltered_listing_sends_nothing() {
        assert!(TransactionFilter::new().to_query().pairs().is_empty());
    }

    #[test]
    fn every_documented_filter_is_reachable() {
        let day = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).expect("a real date");
        let filter = TransactionFilter::new()
            .with_page(PageRequest::first().with_per_page(50))
            .with_sort(TransactionSort::Ascending)
            .with_types(TransactionTypes::One(TransactionType::Trade))
            .with_sub_types(&[TransactionSubType::Fee])
            .with_action(TransactionAction::SellToClose)
            .with_instrument_type(InstrumentType::EquityOption)
            .with_currency("USD")
            .with_symbol("AAPL")
            .with_underlying_symbol("AAPL")
            .with_futures_symbol("/ESU9")
            .with_partition_key("main")
            .with_dates(Some(day(2026, 1, 1)), Some(day(2026, 1, 31)));

        let query = filter.to_query();
        let pairs = query.pairs();

        assert_eq!(pairs[0], ("page-offset", "0"));
        assert_eq!(pairs[1], ("per-page", "50"));
        assert!(pairs.contains(&("sort", "Asc")));
        assert!(pairs.contains(&("type", "Trade")));
        assert!(pairs.contains(&("sub-type[]", "Fee")));
        assert!(pairs.contains(&("action", "Sell to Close")));
        assert!(pairs.contains(&("instrument-type", "Equity Option")));
        assert!(pairs.contains(&("currency", "USD")));
        assert!(pairs.contains(&("futures-symbol", "/ESU9")));
        assert!(pairs.contains(&("partition-key", "main")));
        assert!(pairs.contains(&("start-date", "2026-01-01")));
        assert!(pairs.contains(&("end-date", "2026-01-31")));
    }

    /// The fee total is a magnitude plus a direction, and the direction is not
    /// decoration: a debit of 100 and a credit of 100 are opposite facts.
    #[test]
    fn total_fees_carries_its_direction() {
        let fees: TotalFees =
            serde_json::from_str(r#"{"total-fees": "100.0", "total-fees-effect": "Debit"}"#)
                .expect("the payload from the venue's guide must decode");

        assert_eq!(fees.total_fees.expect("a total").to_string(), "100.0");
        assert_eq!(fees.total_fees_effect, Some(PriceEffect::Debit));
    }
}
