//! Typed filters for the instrument listings.
//!
//! Each of these replaces a method signature that had grown a positional
//! `Option` per query parameter — `list_futures` took five — or that simply
//! could not express the filter at all. Growing a filter is now additive: a new
//! parameter is a new method on the builder, not a new argument every existing
//! caller has to pass `None` for.
//!
//! Every field is optional and an unset one is **omitted** rather than sent
//! empty, because the venue documents its own defaults and a client that sends
//! them all explicitly has replaced them.

use crate::api::query::{PageRequest, QueryBuilder};
use crate::types::order::{AsSymbol, Symbol};
use crate::types::wire::Lendability;

/// Which equities to list, from `GET /instruments/equities`.
///
/// `Default` asks for every equity, one venue-sized page at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquityFilter {
    page: PageRequest,
    symbols: Vec<Symbol>,
    is_etf: Option<bool>,
    is_index: Option<bool>,
    lendability: Option<Lendability>,
}

impl EquityFilter {
    /// Every equity, page by page.
    pub fn new() -> Self {
        Self::default()
    }

    /// The named equities, which is what the endpoint's own summary describes
    /// it as being for.
    pub fn for_symbols(symbols: &[impl AsSymbol]) -> Self {
        Self::new().with_symbols(symbols)
    }

    /// Adds symbols to ask about, sent as repeated `symbol[]` keys.
    #[must_use]
    pub fn with_symbols(mut self, symbols: &[impl AsSymbol]) -> Self {
        self.symbols.extend(symbols.iter().map(AsSymbol::as_symbol));
        self
    }

    /// Restricts to exchange-traded funds, or excludes them.
    #[must_use]
    pub fn with_is_etf(mut self, is_etf: bool) -> Self {
        self.is_etf = Some(is_etf);
        self
    }

    /// Restricts to indices, or excludes them.
    #[must_use]
    pub fn with_is_index(mut self, is_index: bool) -> Self {
        self.is_index = Some(is_index);
        self
    }

    /// Restricts to one borrow classification.
    #[must_use]
    pub fn with_lendability(mut self, lendability: Lendability) -> Self {
        self.lendability = Some(lendability);
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
        query.push_flag("is-etf", self.is_etf);
        query.push_flag("is-index", self.is_index);
        query.push_opt("lendability", self.lendability.as_ref());
        query.push_each(
            "symbol[]",
            self.symbols.iter().map(|symbol| symbol.0.clone()),
        );
        query
    }
}

/// Which active equities to list, from `GET /instruments/equities/active`.
///
/// A separate type from [`EquityFilter`] rather than a flag on it: this
/// endpoint accepts `lendability` and pagination and **nothing else**, and a
/// shared filter would advertise `symbol[]` and `is-etf` parameters that this
/// route ignores.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveEquityFilter {
    page: PageRequest,
    lendability: Option<Lendability>,
}

impl ActiveEquityFilter {
    /// Every active equity, page by page.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts to one borrow classification.
    #[must_use]
    pub fn with_lendability(mut self, lendability: Lendability) -> Self {
        self.lendability = Some(lendability);
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
        query.push_opt("lendability", self.lendability.as_ref());
        query
    }
}

/// Which futures to list, from `GET /instruments/futures`.
///
/// The venue documents `product-code` as an array — `product-code[]=ES&product-code[]=6A`
/// — and it is ignored entirely when `symbol[]` is given. The previous
/// signature could only send one product code, so half of this filter was
/// unreachable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FutureFilter {
    page: PageRequest,
    symbols: Vec<Symbol>,
    product_codes: Vec<String>,
    security_ids: Vec<String>,
    exchange: Option<String>,
    only_active_futures: Option<bool>,
}

impl FutureFilter {
    /// Every future, page by page.
    pub fn new() -> Self {
        Self::default()
    }

    /// The named futures. A leading `/` is not required by the venue.
    pub fn for_symbols(symbols: &[impl AsSymbol]) -> Self {
        Self::new().with_symbols(symbols)
    }

    /// Every future under the given product codes, e.g. `ES` and `6A`.
    ///
    /// The venue ignores this when symbols are also given, which is its
    /// documented behaviour rather than something this crate enforces — a
    /// local rejection would be this library inventing a rule.
    pub fn for_product_codes(codes: &[impl AsRef<str>]) -> Self {
        Self::new().with_product_codes(codes)
    }

    /// Adds symbols, sent as repeated `symbol[]` keys.
    #[must_use]
    pub fn with_symbols(mut self, symbols: &[impl AsSymbol]) -> Self {
        self.symbols.extend(symbols.iter().map(AsSymbol::as_symbol));
        self
    }

    /// Adds product codes, sent as repeated `product-code[]` keys.
    #[must_use]
    pub fn with_product_codes(mut self, codes: &[impl AsRef<str>]) -> Self {
        self.product_codes
            .extend(codes.iter().map(|code| code.as_ref().to_owned()));
        self
    }

    /// Adds exchange-specific routing identifiers, sent as repeated
    /// `security-id[]` keys.
    #[must_use]
    pub fn with_security_ids(mut self, ids: &[impl AsRef<str>]) -> Self {
        self.security_ids
            .extend(ids.iter().map(|id| id.as_ref().to_owned()));
        self
    }

    /// Which exchange, used to disambiguate colliding security identifiers.
    #[must_use]
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// Whether to include futures that are no longer active.
    ///
    /// The venue defaults this to true, so leaving it unset is not the same as
    /// setting it to false.
    #[must_use]
    pub fn with_only_active_futures(mut self, only_active: bool) -> Self {
        self.only_active_futures = Some(only_active);
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
        query.push_opt("exchange", self.exchange.as_ref());
        query.push_flag("only-active-futures", self.only_active_futures);
        query.push_each("product-code[]", self.product_codes.iter().cloned());
        query.push_each("security-id[]", self.security_ids.iter().cloned());
        query.push_each(
            "symbol[]",
            self.symbols.iter().map(|symbol| symbol.0.clone()),
        );
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(query: &QueryBuilder) -> Vec<(String, String)> {
        query
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    /// The default asks for nothing, so the venue's own defaults survive.
    #[test]
    fn an_unfiltered_listing_sends_no_parameters() {
        assert!(rendered(&EquityFilter::new().to_query()).is_empty());
        assert!(rendered(&ActiveEquityFilter::new().to_query()).is_empty());
        assert!(rendered(&FutureFilter::new().to_query()).is_empty());
    }

    #[test]
    fn equity_symbols_are_repeated_keys() {
        let filter = EquityFilter::for_symbols(&["AAPL", "SPY"]);

        assert_eq!(
            rendered(&filter.to_query()),
            vec![
                ("symbol[]".to_string(), "AAPL".to_string()),
                ("symbol[]".to_string(), "SPY".to_string()),
            ]
        );
    }

    #[test]
    fn every_documented_equity_filter_is_reachable() {
        let filter = EquityFilter::new()
            .with_page(PageRequest::new().with_page_offset(2).with_per_page(10))
            .with_is_etf(true)
            .with_is_index(false)
            .with_lendability(Lendability::LocateRequired)
            .with_symbols(&["QQQ"]);

        assert_eq!(
            rendered(&filter.to_query()),
            vec![
                ("page-offset".to_string(), "2".to_string()),
                ("per-page".to_string(), "10".to_string()),
                ("is-etf".to_string(), "true".to_string()),
                ("is-index".to_string(), "false".to_string()),
                ("lendability".to_string(), "Locate Required".to_string()),
                ("symbol[]".to_string(), "QQQ".to_string()),
            ]
        );
    }

    /// The regression this filter exists for: `product-code` was singular and
    /// sent once, so asking about two products was not expressible.
    #[test]
    fn several_product_codes_are_all_sent() {
        let filter = FutureFilter::for_product_codes(&["ES", "6A", "CL"]);

        assert_eq!(
            rendered(&filter.to_query()),
            vec![
                ("product-code[]".to_string(), "ES".to_string()),
                ("product-code[]".to_string(), "6A".to_string()),
                ("product-code[]".to_string(), "CL".to_string()),
            ]
        );
    }

    #[test]
    fn every_documented_future_filter_is_reachable() {
        let filter = FutureFilter::new()
            .with_page(PageRequest::first())
            .with_exchange("CME")
            .with_only_active_futures(false)
            .with_product_codes(&["ES"])
            .with_security_ids(&["12345"])
            .with_symbols(&["ESZ9"]);

        assert_eq!(
            rendered(&filter.to_query()),
            vec![
                ("page-offset".to_string(), "0".to_string()),
                ("exchange".to_string(), "CME".to_string()),
                ("only-active-futures".to_string(), "false".to_string()),
                ("product-code[]".to_string(), "ES".to_string()),
                ("security-id[]".to_string(), "12345".to_string()),
                ("symbol[]".to_string(), "ESZ9".to_string()),
            ]
        );
    }

    /// `only-active-futures` defaults to true at the venue. Not setting it and
    /// setting it to `false` are different requests, and a filter that sent
    /// `false` by default would silently change what every caller receives.
    #[test]
    fn an_unset_flag_is_not_the_same_as_a_false_one() {
        assert!(rendered(&FutureFilter::new().to_query()).is_empty());
        assert_eq!(
            rendered(
                &FutureFilter::new()
                    .with_only_active_futures(false)
                    .to_query()
            ),
            vec![("only-active-futures".to_string(), "false".to_string())]
        );
    }

    /// Lendability goes out as the venue's own text, spaces included.
    #[test]
    fn lendability_is_sent_in_the_venues_spelling() {
        let filter = ActiveEquityFilter::new().with_lendability(Lendability::EasyToBorrow);

        assert_eq!(
            rendered(&filter.to_query()),
            vec![("lendability".to_string(), "Easy To Borrow".to_string())]
        );
    }

    /// A value the venue adds later still round-trips, so a caller is never
    /// stuck waiting for this crate to learn a new classification.
    #[test]
    fn an_unknown_lendability_is_still_expressible() {
        let filter = ActiveEquityFilter::new()
            .with_lendability(Lendability::from("Hard To Borrow".to_string()));

        assert_eq!(
            rendered(&filter.to_query()),
            vec![("lendability".to_string(), "Hard To Borrow".to_string())]
        );
    }

    /// Builders accumulate rather than replace, so a filter can be assembled
    /// across several calls.
    #[test]
    fn symbols_accumulate_across_calls() {
        let filter = EquityFilter::for_symbols(&["AAPL"]).with_symbols(&["MSFT", "SPY"]);

        assert_eq!(
            rendered(&filter.to_query())
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
            vec!["AAPL", "MSFT", "SPY"]
        );
    }
}
