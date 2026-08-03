//! Typed filters for the order endpoints.
//!
//! Four of them, because the venue offers four different parameter sets and a
//! single filter would advertise parameters the route ignores. The history
//! endpoints take date ranges and repeated `status[]`; the live ones take a
//! **single** `status` and nothing else; the customer-scoped pair additionally
//! require `account-numbers[]`, which is why that one cannot be built empty.

use chrono::{DateTime, FixedOffset, NaiveDate};

use crate::accounts::AccountNumber;
use crate::api::query::{PageRequest, QueryBuilder};
use crate::types::instrument::InstrumentType;
use crate::types::order::OrderStatus;

/// Which direction to sort an order listing in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderSort {
    /// Newest first, which is what the venue does when nothing is asked for.
    #[default]
    Descending,
    /// Oldest first.
    Ascending,
}

impl OrderSort {
    /// The text the venue uses.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Descending => "Desc",
            Self::Ascending => "Asc",
        }
    }
}

/// The history filters shared by the account and customer order searches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct HistoryFilters {
    statuses: Vec<OrderStatus>,
    sort: Option<OrderSort>,
    underlying_symbol: Option<String>,
    underlying_instrument_type: Option<InstrumentType>,
    futures_symbol: Option<String>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    start_at: Option<DateTime<FixedOffset>>,
    end_at: Option<DateTime<FixedOffset>>,
}

impl HistoryFilters {
    fn write_into(&self, query: &mut QueryBuilder) {
        query.push_opt("sort", self.sort.map(|sort| sort.as_wire()));
        query.push_each(
            "status[]",
            self.statuses
                .iter()
                .map(|status| status.as_wire().to_string()),
        );
        query.push_opt("underlying-symbol", self.underlying_symbol.as_ref());
        query.push_opt(
            "underlying-instrument-type",
            self.underlying_instrument_type
                .as_ref()
                .map(ToString::to_string),
        );
        query.push_opt("futures-symbol", self.futures_symbol.as_ref());
        query.push_opt("start-date", self.start_date);
        query.push_opt("end-date", self.end_date);
        query.push_opt("start-at", self.start_at.map(|at| at.to_rfc3339()));
        query.push_opt("end-at", self.end_at.map(|at| at.to_rfc3339()));
    }
}

/// Generates the shared history builder methods on a filter type.
///
/// A macro rather than a trait: these are inherent `#[must_use]` builder
/// methods returning `Self`, and a trait would either lose that or force every
/// caller to import it.
macro_rules! history_builders {
    ($name:ident) => {
        impl $name {
            /// Restricts to these statuses, sent as repeated `status[]` keys.
            #[must_use]
            pub fn with_statuses(mut self, statuses: &[OrderStatus]) -> Self {
                self.history.statuses.extend(statuses.iter().cloned());
                self
            }

            /// Which order to sort in.
            #[must_use]
            pub fn with_sort(mut self, sort: OrderSort) -> Self {
                self.history.sort = Some(sort);
                self
            }

            /// Restricts to one underlying.
            #[must_use]
            pub fn with_underlying_symbol(mut self, symbol: impl Into<String>) -> Self {
                self.history.underlying_symbol = Some(symbol.into());
                self
            }

            /// Restricts to one underlying instrument type.
            #[must_use]
            pub fn with_underlying_instrument_type(
                mut self,
                instrument_type: InstrumentType,
            ) -> Self {
                self.history.underlying_instrument_type = Some(instrument_type);
                self
            }

            /// Restricts to one full futures symbol, e.g. `/ESU9`.
            #[must_use]
            pub fn with_futures_symbol(mut self, symbol: impl Into<String>) -> Self {
                self.history.futures_symbol = Some(symbol.into());
                self
            }

            /// Restricts to a range of trading days.
            #[must_use]
            pub fn with_dates(mut self, start: Option<NaiveDate>, end: Option<NaiveDate>) -> Self {
                self.history.start_date = start;
                self.history.end_date = end;
                self
            }

            /// Restricts to a range of instants, offsets preserved.
            #[must_use]
            pub fn with_times(
                mut self,
                start: Option<DateTime<FixedOffset>>,
                end: Option<DateTime<FixedOffset>>,
            ) -> Self {
                self.history.start_at = start;
                self.history.end_at = end;
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
        }
    };
}

/// Which of an account's orders to search, across its whole history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderFilter {
    page: PageRequest,
    history: HistoryFilters,
}

history_builders!(OrderFilter);

impl OrderFilter {
    /// Every order the account has ever had.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        self.page.write_into(&mut query);
        self.history.write_into(&mut query);
        query
    }
}

/// Which of an account's working orders to fetch.
///
/// A separate type from [`OrderFilter`] because the live endpoint accepts
/// pagination, a **single** `status` and an underlying symbol — and nothing
/// else. Reusing the history filter would advertise date ranges and repeated
/// statuses that this route ignores, which reads as a client bug when the
/// results come back unfiltered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveOrderFilter {
    page: PageRequest,
    status: Option<OrderStatus>,
    underlying_symbol: Option<String>,
}

impl LiveOrderFilter {
    /// Every working order.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts to one status. Singular, as the venue documents it.
    #[must_use]
    pub fn with_status(mut self, status: OrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Restricts to one underlying.
    #[must_use]
    pub fn with_underlying_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.underlying_symbol = Some(symbol.into());
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
        query.push_opt("status", self.status.as_ref().map(OrderStatus::as_wire));
        query.push_opt("underlying-symbol", self.underlying_symbol.as_ref());
        query
    }
}

/// Which accounts a customer-scoped order search covers.
///
/// `account-numbers[]` is **required** by the venue, so this cannot be built
/// without one: the constructor takes a first account and any others
/// separately. A `Vec` that happened to be empty would compile and then 400.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerOrderFilter {
    accounts: Vec<AccountNumber>,
    page: PageRequest,
    history: HistoryFilters,
}

history_builders!(CustomerOrderFilter);

impl CustomerOrderFilter {
    /// Orders across `first` and any `rest`.
    pub fn for_accounts(first: impl Into<AccountNumber>, rest: &[AccountNumber]) -> Self {
        let mut accounts = vec![first.into()];
        accounts.extend(rest.iter().cloned());

        Self {
            accounts,
            page: PageRequest::default(),
            history: HistoryFilters::default(),
        }
    }

    /// The accounts this covers. Never empty.
    pub fn accounts(&self) -> &[AccountNumber] {
        &self.accounts
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push_each(
            "account-numbers[]",
            self.accounts.iter().map(|account| account.0.clone()),
        );
        self.page.write_into(&mut query);
        self.history.write_into(&mut query);
        query
    }
}

/// Which accounts a customer-scoped **live** order search covers.
///
/// The live customer endpoint takes `account-numbers[]` and pagination, and
/// nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerLiveOrderFilter {
    accounts: Vec<AccountNumber>,
    page: PageRequest,
}

impl CustomerLiveOrderFilter {
    /// Working orders across `first` and any `rest`.
    pub fn for_accounts(first: impl Into<AccountNumber>, rest: &[AccountNumber]) -> Self {
        let mut accounts = vec![first.into()];
        accounts.extend(rest.iter().cloned());

        Self {
            accounts,
            page: PageRequest::default(),
        }
    }

    /// Which page to ask for.
    #[must_use]
    pub fn with_page(mut self, page: PageRequest) -> Self {
        self.page = page;
        self
    }

    /// The accounts this covers. Never empty.
    pub fn accounts(&self) -> &[AccountNumber] {
        &self.accounts
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push_each(
            "account-numbers[]",
            self.accounts.iter().map(|account| account.0.clone()),
        );
        self.page.write_into(&mut query);
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    #[test]
    fn an_unfiltered_search_sends_nothing() {
        assert!(OrderFilter::new().to_query().pairs().is_empty());
        assert!(LiveOrderFilter::new().to_query().pairs().is_empty());
    }

    #[test]
    fn history_statuses_are_repeated_keys_in_the_venues_spelling() {
        let filter = OrderFilter::new().with_statuses(&[
            OrderStatus::Live,
            OrderStatus::CancelRequested,
            OrderStatus::PartiallyRemoved,
        ]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("status[]", "Live"),
                ("status[]", "Cancel Requested"),
                ("status[]", "Partially Removed"),
            ]
        );
    }

    /// The live endpoint takes a **single** status, not an array. Sending
    /// `status[]` there would be ignored and the caller would get every working
    /// order back believing it was filtered.
    #[test]
    fn the_live_filter_sends_a_single_status() {
        let filter = LiveOrderFilter::new()
            .with_status(OrderStatus::Live)
            .with_underlying_symbol("AAPL");

        assert_eq!(
            filter.to_query().pairs(),
            vec![("status", "Live"), ("underlying-symbol", "AAPL")]
        );
    }

    #[test]
    fn every_documented_history_filter_is_reachable() {
        let filter = OrderFilter::new()
            .with_page(PageRequest::first().with_per_page(25))
            .with_sort(OrderSort::Ascending)
            .with_statuses(&[OrderStatus::Filled])
            .with_underlying_symbol("AAPL")
            .with_underlying_instrument_type(InstrumentType::Equity)
            .with_futures_symbol("/ESU9")
            .with_dates(Some(day(2026, 1, 1)), Some(day(2026, 1, 31)));

        let query = filter.to_query();
        let pairs = query.pairs();

        assert!(pairs.contains(&("page-offset", "0")));
        assert!(pairs.contains(&("per-page", "25")));
        assert!(pairs.contains(&("sort", "Asc")));
        assert!(pairs.contains(&("status[]", "Filled")));
        assert!(pairs.contains(&("underlying-symbol", "AAPL")));
        assert!(pairs.contains(&("underlying-instrument-type", "Equity")));
        assert!(pairs.contains(&("futures-symbol", "/ESU9")));
        assert!(pairs.contains(&("start-date", "2026-01-01")));
        assert!(pairs.contains(&("end-date", "2026-01-31")));
    }

    /// `account-numbers[]` is required, and the constructor makes an empty
    /// selection unrepresentable.
    #[test]
    fn a_customer_search_always_names_at_least_one_account() {
        let one = CustomerOrderFilter::for_accounts("5WX00001", &[]);
        assert_eq!(one.accounts().len(), 1);
        assert_eq!(
            one.to_query().pairs(),
            vec![("account-numbers[]", "5WX00001")]
        );

        let several = CustomerOrderFilter::for_accounts(
            "5WX00001",
            &[
                AccountNumber::from("5WX00002"),
                AccountNumber::from("5WX00003"),
            ],
        );
        assert_eq!(
            several.to_query().pairs(),
            vec![
                ("account-numbers[]", "5WX00001"),
                ("account-numbers[]", "5WX00002"),
                ("account-numbers[]", "5WX00003"),
            ]
        );
    }

    #[test]
    fn the_customer_live_filter_takes_accounts_and_a_page_only() {
        let filter = CustomerLiveOrderFilter::for_accounts("5WX00001", &[])
            .with_page(PageRequest::new().with_per_page(10));

        assert_eq!(
            filter.to_query().pairs(),
            vec![("account-numbers[]", "5WX00001"), ("per-page", "10")]
        );
    }

    /// A status the venue adds later is still expressible, and still
    /// recognisable as unrecognised.
    #[test]
    fn an_unknown_status_round_trips_through_a_filter() {
        let filter =
            OrderFilter::new().with_statuses(&[OrderStatus::from("Something New".to_string())]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![("status[]", "Something New")]
        );
    }
}
