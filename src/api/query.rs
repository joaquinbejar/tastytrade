//! Building query strings, and the page a listing is asked for.
//!
//! The endpoint methods used to assemble `Vec<(&str, &str)>` by hand, which
//! meant every one of them owned a small pile of `String`s purely to keep the
//! borrows alive, and each decided for itself whether an array parameter was
//! `product-code` or `product-code[]`. One of them got that wrong and could
//! only ever send a single product code.
//!
//! Percent-encoding of the values is **not** done here: `reqwest`'s query
//! serializer does it in [`crate::TastyTrade::get_with_query`], which is the
//! right place for it. Path segments are the opposite case — nothing encodes
//! them on the way out — and have their own encoder in `api::url`.

use std::fmt::Display;

/// Accumulates query parameters, owning their rendered values.
///
/// Exists mostly so repeated keys have one implementation. The venue spells an
/// array parameter `symbol[]=A&symbol[]=B` — the same key sent again, brackets
/// included — and a caller that sends `symbol=A,B` gets a search for one
/// instrument whose symbol contains a comma.
#[derive(Debug, Default)]
pub(crate) struct QueryBuilder {
    pairs: Vec<(&'static str, String)>,
}

impl QueryBuilder {
    /// An empty query.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Adds `key=value`.
    pub(crate) fn push(&mut self, key: &'static str, value: impl Display) {
        self.pairs.push((key, value.to_string()));
    }

    /// Adds `key=value` when there is a value.
    ///
    /// An absent optional parameter must be **absent**, not sent empty: the
    /// venue documents defaults for several of these, and `only-active-futures`
    /// defaults to true. Sending the key with nothing after it is a different
    /// request from not sending it.
    pub(crate) fn push_opt(&mut self, key: &'static str, value: Option<impl Display>) {
        if let Some(value) = value {
            self.push(key, value);
        }
    }

    /// Adds `key=true` or `key=false` when the flag is set.
    ///
    /// Separate from [`QueryBuilder::push_opt`] because Rust's `bool` renders
    /// as `true`/`false` and it would be easy to reach for `1`/`0` at one call
    /// site out of six.
    pub(crate) fn push_flag(&mut self, key: &'static str, value: Option<bool>) {
        self.push_opt(key, value);
    }

    /// Adds `key` once per value, which is how the venue spells an array.
    ///
    /// `key` is expected to already carry its `[]` suffix, because that suffix
    /// is part of the parameter name the venue documents rather than something
    /// this function should be inventing.
    pub(crate) fn push_each(
        &mut self,
        key: &'static str,
        values: impl IntoIterator<Item = String>,
    ) {
        for value in values {
            self.pairs.push((key, value));
        }
    }

    /// The pairs, in the order they were added, borrowed for the request.
    pub(crate) fn pairs(&self) -> Vec<(&str, &str)> {
        self.pairs
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect()
    }
}

/// Which page of a paginated listing to ask for.
///
/// Both fields are optional and both are omitted when unset, so
/// `PageRequest::default()` asks for whatever the venue considers the first
/// page at its own default size. That matters: the venue's defaults are part
/// of its contract, and a client that always sends `page-offset=0&per-page=1000`
/// has quietly replaced them with its own.
///
/// The response side is [`crate::api::base::Pagination`], which reports where
/// the page actually landed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageRequest {
    page_offset: Option<u32>,
    per_page: Option<u32>,
}

impl PageRequest {
    /// A request that sends no pagination parameters at all.
    pub fn new() -> Self {
        Self::default()
    }

    /// The first page, at the venue's default size.
    pub fn first() -> Self {
        Self::new().with_page_offset(0)
    }

    /// Which page, counting from zero.
    #[must_use]
    pub fn with_page_offset(mut self, page_offset: u32) -> Self {
        self.page_offset = Some(page_offset);
        self
    }

    /// How many items per page.
    #[must_use]
    pub fn with_per_page(mut self, per_page: u32) -> Self {
        self.per_page = Some(per_page);
        self
    }

    /// The page offset this asks for, when it asks for one.
    pub fn page_offset(&self) -> Option<u32> {
        self.page_offset
    }

    /// The page size this asks for, when it asks for one.
    pub fn per_page(&self) -> Option<u32> {
        self.per_page
    }

    /// The page after this one, keeping the same size.
    ///
    /// An unset offset means "the venue's first page", so the next one is page
    /// one. Saturating rather than wrapping, because a wrapped offset would
    /// silently restart the listing.
    #[must_use]
    pub fn next_page(self) -> Self {
        Self {
            page_offset: Some(self.page_offset.unwrap_or(0).saturating_add(1)),
            ..self
        }
    }

    /// Writes this page's parameters into `query`.
    pub(crate) fn write_into(&self, query: &mut QueryBuilder) {
        query.push_opt("page-offset", self.page_offset);
        query.push_opt("per-page", self.per_page);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_optional_parameter_is_absent_rather_than_empty() {
        let mut query = QueryBuilder::new();
        query.push_opt("per-page", None::<u32>);
        query.push_flag("is-etf", None);

        assert!(query.pairs().is_empty());
    }

    /// The bug this replaces: `product-code` was sent once with one value, so
    /// a caller could not ask about two products.
    #[test]
    fn an_array_parameter_repeats_its_key() {
        let mut query = QueryBuilder::new();
        query.push_each(
            "product-code[]",
            ["ES".to_string(), "6A".to_string(), "CL".to_string()],
        );

        assert_eq!(
            query.pairs(),
            vec![
                ("product-code[]", "ES"),
                ("product-code[]", "6A"),
                ("product-code[]", "CL"),
            ]
        );
    }

    #[test]
    fn an_empty_array_parameter_adds_nothing() {
        let mut query = QueryBuilder::new();
        query.push_each("symbol[]", Vec::new());

        assert!(query.pairs().is_empty());
    }

    #[test]
    fn a_flag_renders_as_true_or_false() {
        let mut query = QueryBuilder::new();
        query.push_flag("is-etf", Some(true));
        query.push_flag("is-index", Some(false));

        assert_eq!(
            query.pairs(),
            vec![("is-etf", "true"), ("is-index", "false")]
        );
    }

    /// Order is preserved, which is what makes a test able to assert on the
    /// whole query string rather than on set membership.
    #[test]
    fn parameters_keep_the_order_they_were_added_in() {
        let mut query = QueryBuilder::new();
        query.push("a", 1);
        query.push_each("b[]", ["x".to_string()]);
        query.push("c", "z");

        assert_eq!(query.pairs(), vec![("a", "1"), ("b[]", "x"), ("c", "z")]);
    }

    #[test]
    fn a_default_page_request_sends_nothing() {
        let mut query = QueryBuilder::new();
        PageRequest::default().write_into(&mut query);

        assert!(
            query.pairs().is_empty(),
            "an unset page must leave the venue's own defaults alone"
        );
    }

    #[test]
    fn a_page_request_sends_only_what_it_was_given() {
        let mut query = QueryBuilder::new();
        PageRequest::new().with_per_page(50).write_into(&mut query);

        assert_eq!(query.pairs(), vec![("per-page", "50")]);
    }

    #[test]
    fn the_next_page_keeps_the_size_and_advances_the_offset() {
        let page = PageRequest::new().with_per_page(25).with_page_offset(3);
        let next = page.next_page();

        assert_eq!(next.page_offset(), Some(4));
        assert_eq!(next.per_page(), Some(25));

        // An unset offset means the venue's first page, so the next one is one.
        assert_eq!(PageRequest::new().next_page().page_offset(), Some(1));
    }

    /// Saturating, not wrapping: a wrapped offset restarts the listing, and a
    /// caller paging in a loop would never terminate.
    #[test]
    fn advancing_past_the_end_saturates() {
        let page = PageRequest::new().with_page_offset(u32::MAX);

        assert_eq!(page.next_page().page_offset(), Some(u32::MAX));
    }
}
