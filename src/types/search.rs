//! Finding an instrument you cannot already name.
//!
//! Two searches with different shapes. `GET /symbols/search/{symbol}` is a
//! prefix search over symbols and company names, answering with enough to put
//! in front of a person. `GET /instruments/search` searches symbols,
//! descriptions and tags across every instrument type, with classification
//! filters.
//!
//! Neither swagger marks a single field as required, so every field here is
//! `Option<T>` except the symbol itself: a search result with no symbol cannot
//! be acted on, and `Items<T>` recording it as skipped is a better answer than
//! a struct full of `None`.

use std::fmt;

use chrono::{DateTime, FixedOffset, NaiveDate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::query::QueryBuilder;

/// One row from `GET /symbols/search/{symbol}`.
///
/// The prefix search. `price_increments` and `trading_hours` are typed as
/// strings by the venue and are left that way: no captured payload shows what
/// is in them, and a structured type invented from a field name is a type that
/// stops decoding the first time it is wrong.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct SymbolSearchResult {
    /// The symbol, which is what you came for.
    pub symbol: String,
    /// The company or contract name.
    pub description: Option<String>,
    /// Where it is listed.
    pub listed_market: Option<String>,
    /// Price increments, as the venue renders them.
    pub price_increments: Option<String>,
    /// Trading hours, as the venue renders them.
    pub trading_hours: Option<String>,
    /// Whether it has listed options. `None` when the venue did not say, which
    /// is not the same as "no".
    pub options: Option<bool>,
    /// Equity, Future, Cryptocurrency and so on.
    pub instrument_type: Option<String>,
}

/// One row from `GET /instruments/search`.
///
/// Broader than [`SymbolSearchResult`]: it spans every instrument type and
/// carries the classification the filters select on, so a caller can show why
/// a row matched.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct InstrumentSearchResult {
    /// The symbol.
    pub symbol: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Classification, e.g. an event or a sector.
    pub category: Option<String>,
    /// Finer classification within [`InstrumentSearchResult::category`].
    pub sub_category: Option<String>,
    /// Listing exchange.
    pub exchange: Option<String>,
    /// Equity, Future, Cryptocurrency and so on.
    pub instrument_type: Option<String>,
    /// Venue-side identifier.
    pub external_id: Option<String>,
    /// Identifier of the event product, for event contracts.
    pub event_product_external_id: Option<String>,
    /// Which strike shapes the instrument offers.
    pub strike_types: Option<String>,
    /// The product the instrument is written on.
    pub underlying_product: Option<String>,
    /// That product's type.
    pub underlying_product_type: Option<String>,
    /// What to subscribe to on the streamer for the underlying.
    pub underlying_streamer_symbol: Option<String>,
    /// When the instrument stops trading, offset preserved.
    #[serde(default, with = "crate::types::wire::datetime_option")]
    pub stops_trading_at: Option<DateTime<FixedOffset>>,
}

/// The largest `limit` this crate will send.
///
/// Not in the swagger, which describes `limit` only as "Maximum number of
/// results". The cap is the documented one from the API guide, enforced here
/// so an over-large request fails in the caller's process rather than at the
/// venue — and the error names the cap, so it is obvious the refusal is this
/// crate's rule and not a network problem.
pub const MAX_SEARCH_RESULTS: u32 = 100;

/// What to search `GET /instruments/search` for.
///
/// Every classification filter is **comma-separated**, not a repeated key —
/// the opposite of the instrument listings, and the venue's choice rather than
/// this crate's. Getting it wrong returns results for one value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstrumentSearchFilter {
    query: Option<String>,
    types: Vec<String>,
    categories: Vec<String>,
    exchanges: Vec<String>,
    sub_types: Vec<String>,
    from_date: Option<NaiveDate>,
    limit: Option<u32>,
}

impl InstrumentSearchFilter {
    /// An unfiltered search, which the venue is free to answer as it likes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Searches symbols, descriptions and tags for `query`.
    pub fn for_query(query: impl Into<String>) -> Self {
        Self::new().with_query(query)
    }

    /// Sets the search text.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Restricts to instrument types, comma-joined into one `type` parameter.
    #[must_use]
    pub fn with_types(mut self, types: &[impl AsRef<str>]) -> Self {
        self.types
            .extend(types.iter().map(|value| value.as_ref().to_owned()));
        self
    }

    /// Restricts to categories, comma-joined into one `category` parameter.
    #[must_use]
    pub fn with_categories(mut self, categories: &[impl AsRef<str>]) -> Self {
        self.categories
            .extend(categories.iter().map(|value| value.as_ref().to_owned()));
        self
    }

    /// Restricts to exchanges, comma-joined into one `exchange` parameter.
    #[must_use]
    pub fn with_exchanges(mut self, exchanges: &[impl AsRef<str>]) -> Self {
        self.exchanges
            .extend(exchanges.iter().map(|value| value.as_ref().to_owned()));
        self
    }

    /// Restricts to equity sub-types — the venue documents `ETF` and `Index`.
    #[must_use]
    pub fn with_instrument_sub_types(mut self, sub_types: &[impl AsRef<str>]) -> Self {
        self.sub_types
            .extend(sub_types.iter().map(|value| value.as_ref().to_owned()));
        self
    }

    /// Lower bound on when an instrument stops trading, which is what surfaces
    /// expired events.
    #[must_use]
    pub fn with_from_date(mut self, from_date: NaiveDate) -> Self {
        self.from_date = Some(from_date);
        self
    }

    /// How many results at most.
    ///
    /// Checked when the search runs, not here: a builder that returned
    /// `Result` would put a `?` on every call in the chain for one parameter.
    #[must_use]
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// The limit this filter asks for.
    pub fn limit(&self) -> Option<u32> {
        self.limit
    }

    /// Fails when the limit exceeds what the venue accepts.
    ///
    /// [`crate::TastyTradeError::Precondition`], so `is_retryable()` is false:
    /// nothing was sent and sending it again would fail the same way.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        if let Some(limit) = self.limit
            && limit > MAX_SEARCH_RESULTS
        {
            return Err(crate::TastyTradeError::Precondition(format!(
                "instrument search accepts at most {MAX_SEARCH_RESULTS} results, \
                 and {limit} were asked for; lower the limit or page through \
                 the listing endpoints instead"
            )));
        }
        Ok(())
    }

    pub(crate) fn to_query(&self) -> QueryBuilder {
        let mut query = QueryBuilder::new();
        query.push_opt("query", self.query.as_ref());
        push_joined(&mut query, "type", &self.types);
        push_joined(&mut query, "category", &self.categories);
        push_joined(&mut query, "exchange", &self.exchanges);
        push_joined(&mut query, "instrument-sub-type", &self.sub_types);
        query.push_opt("from-date", self.from_date.map(|date| date.to_string()));
        query.push_opt("limit", self.limit);
        query
    }
}

/// Adds `key=a,b,c`, or nothing when there is nothing to add.
fn push_joined(query: &mut QueryBuilder, key: &'static str, values: &[String]) {
    if !values.is_empty() {
        query.push(key, values.join(","));
    }
}

/// A short-lived third-party client token for AI search.
///
/// **This is a credential.** It is treated like the DXLink quote token: never
/// in `Debug`, `Display`, a log line or an error, and reaching the value takes
/// [`AiSearchToken::expose`], which is one grep away from an audit.
///
/// The whole `data` object is kept rather than a named field, because the
/// published spec documents the endpoint's path and method and **no response
/// schema at all** — its `200` is an empty object. Guessing a field name would
/// make the token silently `None` the moment the guess was wrong, and a token
/// that quietly turns into nothing is worse than one a caller has to read a
/// key out of. [`AiSearchToken::field`] is that escape hatch.
///
/// The service it authenticates is not part of the tastytrade API this crate
/// wraps, so the token is minted and handed back rather than used here.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct AiSearchToken {
    raw: Value,
}

impl AiSearchToken {
    /// The whole response object, as it arrived.
    ///
    /// Every call is a place a credential can leave the process.
    pub fn expose(&self) -> &Value {
        &self.raw
    }

    /// One field of the response, by the venue's own key.
    ///
    /// For when the response shape is known — from a capture, or from
    /// Telescope's own documentation — and this crate's model is not the place
    /// to encode it.
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.raw.get(name)
    }

    /// When the token stops working, if the venue said.
    ///
    /// A **probe**, not a contract: it looks for `expires-at` and then
    /// `expires_at`, because the venue spells its JSON in kebab-case and RFC
    /// 6749 responses in snake_case and this endpoint publishes neither. `None`
    /// means the field was absent or unparseable, never that the token does not
    /// expire — it does, and the whole point of the endpoint is that it is
    /// short-lived.
    pub fn expires_at(&self) -> Option<DateTime<FixedOffset>> {
        ["expires-at", "expires_at"]
            .iter()
            .find_map(|key| self.raw.get(*key))
            .and_then(Value::as_str)
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
    }

    /// How many bytes the response was. Safe to log; the contents are not.
    pub fn len(&self) -> usize {
        self.raw.to_string().len()
    }

    /// Whether the venue answered with nothing usable.
    pub fn is_empty(&self) -> bool {
        match &self.raw {
            Value::Null => true,
            Value::Object(map) => map.is_empty(),
            _ => false,
        }
    }
}

impl fmt::Debug for AiSearchToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AiSearchToken(<redacted, {} bytes>)", self.len())
    }
}

impl fmt::Display for AiSearchToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<redacted, {} bytes>", self.len())
    }
}

// Serialising a credential is how it ends up in a file, so the round trip the
// generic verbs need is provided explicitly and is the only one there is.
impl Serialize for AiSearchToken {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value that must never appear anywhere but `expose`.
    const SENTINEL: &str = "SENTINEL-telescope-token-3Qv7";

    fn token() -> AiSearchToken {
        serde_json::from_str(&format!(
            r#"{{"token":"{SENTINEL}","expires-at":"2026-08-03T12:00:00.000+00:00"}}"#
        ))
        .expect("valid JSON")
    }

    #[test]
    fn the_ai_search_token_never_renders_itself() {
        let token = token();
        let rendered = format!("{token:?} {token} {}", format_args!("{token:#?}"));

        assert!(
            !rendered.contains(SENTINEL),
            "the token reached a rendering: {rendered}"
        );
        assert!(rendered.contains("redacted"), "{rendered}");
    }

    /// The one place it can leave the process, and it still works.
    #[test]
    fn exposing_it_returns_what_arrived() {
        assert_eq!(
            token().field("token").and_then(Value::as_str),
            Some(SENTINEL)
        );
        assert_eq!(
            token().expose().get("token").and_then(Value::as_str),
            Some(SENTINEL)
        );
    }

    /// An error built from a token must not render it either — a `Precondition`
    /// message is a string a caller prints.
    #[test]
    fn a_token_inside_an_error_message_stays_redacted() {
        let error = crate::TastyTradeError::Precondition(format!("minted {}", token()));

        assert!(!format!("{error} {error:?}").contains(SENTINEL));
    }

    #[test]
    fn the_expiry_probe_finds_both_spellings_and_invents_nothing() {
        assert!(token().expires_at().is_some());

        let snake: AiSearchToken =
            serde_json::from_str(r#"{"expires_at":"2026-08-03T12:00:00.000+00:00"}"#)
                .expect("valid JSON");
        assert!(snake.expires_at().is_some());

        let absent: AiSearchToken = serde_json::from_str(r#"{"token":"x"}"#).expect("valid JSON");
        assert_eq!(absent.expires_at(), None);

        // Not a date, so not a date. The probe must not produce a plausible
        // wrong answer.
        let garbage: AiSearchToken =
            serde_json::from_str(r#"{"expires-at":"soon"}"#).expect("valid JSON");
        assert_eq!(garbage.expires_at(), None);
    }

    #[test]
    fn an_empty_answer_is_recognisable() {
        let empty: AiSearchToken = serde_json::from_str("{}").expect("valid JSON");
        assert!(empty.is_empty());
        assert!(!token().is_empty());
    }

    /// The classification filters are comma-joined into one parameter each,
    /// which is the opposite of the instrument listings' repeated keys. Sending
    /// them repeated returns results for one value.
    #[test]
    fn classification_filters_are_comma_joined_into_one_parameter() {
        let filter = InstrumentSearchFilter::for_query("apple")
            .with_types(&["Equity", "Equity Option"])
            .with_instrument_sub_types(&["ETF", "Index"]);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("query", "apple"),
                ("type", "Equity,Equity Option"),
                ("instrument-sub-type", "ETF,Index"),
            ]
        );
    }

    #[test]
    fn every_documented_search_parameter_is_reachable() {
        let filter = InstrumentSearchFilter::for_query("gold")
            .with_types(&["Future"])
            .with_categories(&["Metals"])
            .with_exchanges(&["CME"])
            .with_instrument_sub_types(&["Index"])
            .with_from_date(NaiveDate::from_ymd_opt(2026, 1, 31).expect("a real date"))
            .with_limit(10);

        assert_eq!(
            filter.to_query().pairs(),
            vec![
                ("query", "gold"),
                ("type", "Future"),
                ("category", "Metals"),
                ("exchange", "CME"),
                ("instrument-sub-type", "Index"),
                ("from-date", "2026-01-31"),
                ("limit", "10"),
            ]
        );
    }

    #[test]
    fn an_empty_filter_sends_nothing() {
        assert!(InstrumentSearchFilter::new().to_query().pairs().is_empty());
    }

    #[test]
    fn an_over_large_limit_fails_locally_and_is_not_retryable() {
        let filter = InstrumentSearchFilter::new().with_limit(MAX_SEARCH_RESULTS + 1);

        let error = filter
            .validate()
            .expect_err("the cap must be enforced before anything is sent");

        assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
        assert!(
            !error.is_retryable(),
            "a local refusal sent nothing, so retrying it changes nothing"
        );
        assert!(
            format!("{error}").contains("100"),
            "the message must name the cap: {error}"
        );
    }

    #[test]
    fn the_cap_itself_is_accepted() {
        assert!(
            InstrumentSearchFilter::new()
                .with_limit(MAX_SEARCH_RESULTS)
                .validate()
                .is_ok()
        );
    }

    /// A search row keeps the offset the venue sent rather than being
    /// normalised, per the crate's date-time rule.
    #[test]
    fn a_search_result_decodes_and_keeps_its_offset() {
        let row: InstrumentSearchResult = serde_json::from_str(
            r#"{
                "symbol": "/ESZ4",
                "description": "E-mini S&P 500",
                "category": "Equity Index",
                "instrument-type": "Future",
                "stops-trading-at": "2026-12-19T14:30:00.000-05:00"
            }"#,
        )
        .expect("the row must decode");

        assert_eq!(row.symbol, "/ESZ4");
        assert_eq!(row.exchange, None, "an absent field is absent, not empty");
        assert_eq!(
            row.stops_trading_at
                .expect("a timestamp")
                .offset()
                .local_minus_utc(),
            -5 * 3600
        );
    }

    /// Nothing but the symbol is required, which is what a search row that
    /// carries only a match actually looks like.
    #[test]
    fn a_symbol_search_row_needs_only_its_symbol() {
        let row: SymbolSearchResult =
            serde_json::from_str(r#"{"symbol":"AAPL"}"#).expect("the row must decode");

        assert_eq!(row.symbol, "AAPL");
        assert_eq!(row.options, None, "an omitted flag is unknown, not false");
        assert_eq!(row.description, None);
    }
}
