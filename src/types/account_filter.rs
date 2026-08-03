//! Typed filters for the account-scoped listings.
//!
//! Two problems these solve. The snapshot endpoint accepts a **single day** and
//! a **date range** as separate parameters, and sending both is a query that
//! contradicts itself — so the two are one enum here and the contradiction
//! cannot be written. Positions accept eight filters and the crate sent none,
//! so every caller fetched the whole book and filtered it locally.

use chrono::NaiveDate;

use crate::api::query::{PageRequest, QueryBuilder};
use crate::types::balance::SnapshotTimeOfDay;
use crate::types::instrument::InstrumentType;
use crate::types::order::{AsSymbol, Symbol};

/// Which snapshots to fetch: one day, or a range.
///
/// An enum rather than three optional fields, because `snapshot-date` and
/// `start-date`/`end-date` are alternatives and a request carrying both is one
/// the venue has to resolve however it likes. Making it unrepresentable is
/// cheaper than documenting which one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotRange {
    /// A single day, sent as `snapshot-date`.
    OnDate(NaiveDate),
    /// A range, sent as `start-date` and `end-date`. Either end may be open,
    /// which is what the venue's two independent parameters allow.
    Range {
        /// First day to include.
        start: Option<NaiveDate>,
        /// Last day to include.
        end: Option<NaiveDate>,
    },
}

impl SnapshotRange {
    /// One day.
    pub fn on(date: NaiveDate) -> Self {
        Self::OnDate(date)
    }

    /// A closed range.
    pub fn between(start: NaiveDate, end: NaiveDate) -> Self {
        Self::Range {
            start: Some(start),
            end: Some(end),
        }
    }

    /// From a day onwards.
    pub fn from(start: NaiveDate) -> Self {
        Self::Range {
            start: Some(start),
            end: None,
        }
    }

    /// Up to and including a day.
    pub fn until(end: NaiveDate) -> Self {
        Self::Range {
            start: None,
            end: Some(end),
        }
    }

    fn write_into(&self, query: &mut QueryBuilder) {
        match self {
            Self::OnDate(date) => query.push("snapshot-date", date),
            Self::Range { start, end } => {
                query.push_opt("start-date", *start);
                query.push_opt("end-date", *end);
            }
        }
    }
}

impl Default for SnapshotRange {
    /// Whatever the venue considers recent: neither end bounded.
    fn default() -> Self {
        Self::Range {
            start: None,
            end: None,
        }
    }
}

/// Which balance snapshots to fetch.
///
/// [`SnapshotTimeOfDay`] is a constructor argument rather than an optional
/// field because the venue marks `time-of-day` **required**. A filter that
/// could omit it would compile and then 400.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceSnapshotFilter {
    time_of_day: SnapshotTimeOfDay,
    range: SnapshotRange,
    currency: Option<String>,
    page: PageRequest,
}

impl BalanceSnapshotFilter {
    /// Snapshots taken at `time_of_day`, over whatever range the venue
    /// defaults to.
    pub fn at(time_of_day: SnapshotTimeOfDay) -> Self {
        Self {
            time_of_day,
            range: SnapshotRange::default(),
            currency: None,
            page: PageRequest::default(),
        }
    }

    /// Which days to cover.
    #[must_use]
    pub fn with_range(mut self, range: SnapshotRange) -> Self {
        self.range = range;
        self
    }

    /// Restricts to one currency.
    #[must_use]
    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
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

    /// Which end of the day these snapshots are from.
    pub fn time_of_day(&self) -> SnapshotTimeOfDay {
        self.time_of_day
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        self.page.write_into(&mut query);
        // `as_wire`, not `Display`-by-accident: the venue spells these `EOD`
        // and `BOD`.
        query.push("time-of-day", self.time_of_day.as_wire());
        query.push_opt("currency", self.currency.as_ref());
        self.range.write_into(&mut query);
        query
    }
}

/// Which positions to fetch.
///
/// `Default` is every open position, which is what the unfiltered
/// [`crate::accounts::Account::positions`] has always asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PositionFilter {
    include_closed_positions: Option<bool>,
    include_marks: Option<bool>,
    net_positions: Option<bool>,
    instrument_type: Option<InstrumentType>,
    symbol: Option<Symbol>,
    underlying_symbols: Vec<Symbol>,
    underlying_product_code: Option<String>,
    partition_keys: Vec<String>,
}

impl PositionFilter {
    /// Every open position.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to include positions that have been closed.
    #[must_use]
    pub fn with_closed_positions(mut self, include: bool) -> Self {
        self.include_closed_positions = Some(include);
        self
    }

    /// Whether to include the current quote mark.
    ///
    /// The venue warns this can slow the request down. What comes back lands
    /// in [`crate::FullPosition::mark`] and
    /// [`crate::FullPosition::mark_price`].
    #[must_use]
    pub fn with_marks(mut self, include: bool) -> Self {
        self.include_marks = Some(include);
        self
    }

    /// Whether to net positions by instrument type and symbol rather than
    /// listing each lot.
    #[must_use]
    pub fn with_net_positions(mut self, net: bool) -> Self {
        self.net_positions = Some(net);
        self
    }

    /// Restricts to one instrument type.
    #[must_use]
    pub fn with_instrument_type(mut self, instrument_type: InstrumentType) -> Self {
        self.instrument_type = Some(instrument_type);
        self
    }

    /// Restricts to one exact symbol — a ticker, an OCC option symbol, or a
    /// futures symbol.
    #[must_use]
    pub fn with_symbol(mut self, symbol: impl AsSymbol) -> Self {
        self.symbol = Some(symbol.as_symbol());
        self
    }

    /// Restricts to positions on the given underlyings, sent as repeated
    /// `underlying-symbol[]` keys.
    #[must_use]
    pub fn with_underlying_symbols(mut self, symbols: &[impl AsSymbol]) -> Self {
        self.underlying_symbols
            .extend(symbols.iter().map(AsSymbol::as_symbol));
        self
    }

    /// Restricts to one futures product code, e.g. `ES`.
    #[must_use]
    pub fn with_underlying_product_code(mut self, code: impl Into<String>) -> Self {
        self.underlying_product_code = Some(code.into());
        self
    }

    /// Restricts to account partitions, sent as repeated `partition-keys[]`.
    #[must_use]
    pub fn with_partition_keys(mut self, keys: &[impl AsRef<str>]) -> Self {
        self.partition_keys
            .extend(keys.iter().map(|key| key.as_ref().to_owned()));
        self
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push_flag("include-closed-positions", self.include_closed_positions);
        query.push_flag("include-marks", self.include_marks);
        query.push_flag("net-positions", self.net_positions);
        query.push_opt(
            "instrument-type",
            self.instrument_type.as_ref().map(ToString::to_string),
        );
        query.push_opt("symbol", self.symbol.as_ref().map(|symbol| &symbol.0));
        query.push_opt(
            "underlying-product-code",
            self.underlying_product_code.as_ref(),
        );
        query.push_each("partition-keys[]", self.partition_keys.iter().cloned());
        query.push_each(
            "underlying-symbol[]",
            self.underlying_symbols
                .iter()
                .map(|symbol| symbol.0.clone()),
        );
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a real date")
    }

    /// The required parameter, spelled the way the venue spells it. It used to
    /// go out as `Eod`, taken from a `Display` that was really the derived
    /// `Debug`.
    #[test]
    fn the_required_time_of_day_uses_the_venues_spelling() {
        let filter = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod);

        assert_eq!(filter.to_query().pairs(), vec![("time-of-day", "EOD")]);
        assert_eq!(
            BalanceSnapshotFilter::at(SnapshotTimeOfDay::Bod)
                .to_query()
                .pairs(),
            vec![("time-of-day", "BOD")]
        );
    }

    /// A single day and a range are alternatives, so only one set of keys can
    /// ever be produced. This is the contradiction the enum removes.
    #[test]
    fn a_single_day_and_a_range_can_never_be_sent_together() {
        let one_day = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
            .with_range(SnapshotRange::on(day(2026, 3, 14)));
        let query = one_day.to_query();
        let pairs = query.pairs();

        assert!(pairs.contains(&("snapshot-date", "2026-03-14")));
        assert!(pairs.iter().all(|(key, _)| *key != "start-date"));
        assert!(pairs.iter().all(|(key, _)| *key != "end-date"));

        let ranged = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
            .with_range(SnapshotRange::between(day(2026, 3, 1), day(2026, 3, 31)));
        let query = ranged.to_query();
        let pairs = query.pairs();

        assert!(pairs.contains(&("start-date", "2026-03-01")));
        assert!(pairs.contains(&("end-date", "2026-03-31")));
        assert!(pairs.iter().all(|(key, _)| *key != "snapshot-date"));
    }

    /// Either end of a range may be open, which is what two independent
    /// optional parameters allow.
    #[test]
    fn a_half_open_range_sends_only_the_end_it_has() {
        let from = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
            .with_range(SnapshotRange::from(day(2026, 3, 1)));
        assert_eq!(
            from.to_query().pairs(),
            vec![("time-of-day", "EOD"), ("start-date", "2026-03-01")]
        );

        let until = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
            .with_range(SnapshotRange::until(day(2026, 3, 31)));
        assert_eq!(
            until.to_query().pairs(),
            vec![("time-of-day", "EOD"), ("end-date", "2026-03-31")]
        );
    }

    #[test]
    fn every_documented_snapshot_parameter_is_reachable() {
        let filter = BalanceSnapshotFilter::at(SnapshotTimeOfDay::Bod)
            .with_page(PageRequest::new().with_page_offset(2).with_per_page(10))
            .with_currency("USD")
            .with_range(SnapshotRange::between(day(2026, 1, 1), day(2026, 1, 31)));

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("page-offset", "2"),
                ("per-page", "10"),
                ("time-of-day", "BOD"),
                ("currency", "USD"),
                ("start-date", "2026-01-01"),
                ("end-date", "2026-01-31"),
            ]
        );
    }

    /// The default sends nothing, so an unfiltered call is byte-for-byte the
    /// request `positions()` has always made.
    #[test]
    fn an_unfiltered_position_query_is_empty() {
        assert!(PositionFilter::new().to_query().pairs().is_empty());
    }

    #[test]
    fn position_arrays_are_repeated_keys() {
        let filter = PositionFilter::new()
            .with_underlying_symbols(&["AAPL", "SPY"])
            .with_partition_keys(&["main", "ira"]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("partition-keys[]", "main"),
                ("partition-keys[]", "ira"),
                ("underlying-symbol[]", "AAPL"),
                ("underlying-symbol[]", "SPY"),
            ]
        );
    }

    #[test]
    fn every_documented_position_filter_is_reachable() {
        let filter = PositionFilter::new()
            .with_closed_positions(true)
            .with_marks(true)
            .with_net_positions(false)
            .with_instrument_type(InstrumentType::Equity)
            .with_symbol("AAPL")
            .with_underlying_product_code("ES")
            .with_underlying_symbols(&["AAPL"])
            .with_partition_keys(&["main"]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("include-closed-positions", "true"),
                ("include-marks", "true"),
                ("net-positions", "false"),
                ("instrument-type", "Equity"),
                ("symbol", "AAPL"),
                ("underlying-product-code", "ES"),
                ("partition-keys[]", "main"),
                ("underlying-symbol[]", "AAPL"),
            ]
        );
    }

    /// An unset flag is omitted, not sent as `false`. The venue decides what an
    /// absent `include-closed-positions` means, and this crate does not get to
    /// answer for it.
    #[test]
    fn an_unset_position_flag_is_omitted() {
        assert!(
            PositionFilter::new()
                .to_query()
                .pairs()
                .iter()
                .all(|(key, _)| *key != "include-closed-positions")
        );
        assert_eq!(
            PositionFilter::new()
                .with_closed_positions(false)
                .to_query()
                .pairs(),
            vec![("include-closed-positions", "false")]
        );
    }
}
