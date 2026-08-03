//! Watchlists, as the account streamer and the REST endpoints both see them.
//!
//! `public-watchlists-subscribe` publishes tastytrade's own curated lists when
//! they change. The same object is what `GET /public-watchlists` and
//! `GET /watchlists` return, so the type lives here rather than in the
//! streaming module: when the REST side lands (#80) it reuses this rather than
//! declaring a second shape for the same wire object.

use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};

use crate::types::order::Symbol;

/// A named list of instruments.
///
/// `name` is the only field the venue's own create schema marks required, and
/// it is the identity of the list, so it is the only one that is not
/// `Option` here.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Watchlist {
    /// The list's name, which is also how it is addressed in a URL.
    pub name: String,
    /// The instruments on it.
    ///
    /// Defaulted rather than required: an empty list is a list, and a
    /// notification about one that was just emptied must still be delivered.
    #[serde(default)]
    pub watchlist_entries: Vec<WatchlistEntry>,
    /// The group the list belongs to, for the curated ones.
    #[serde(default)]
    pub group_name: Option<String>,
    /// Where it sorts among its siblings.
    #[serde(default)]
    pub order_index: Option<i32>,
    /// The content-management identifier, on tastytrade's own lists.
    #[serde(default)]
    pub cms_id: Option<String>,
}

/// One instrument on a watchlist.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct WatchlistEntry {
    /// The instrument symbol. Required by the venue's schema.
    pub symbol: Symbol,
    /// What kind of instrument it is, when the venue says.
    ///
    /// `String`, not the crate's [`InstrumentType`](crate::InstrumentType),
    /// for the same reason as [`QuoteAlert::instrument_type`]: that enum is a
    /// closed set with no unknown arm, and tastytrade's curated lists are
    /// exactly where an instrument type this crate has not modelled would turn
    /// up first. One unrecognised entry would cost the caller the whole
    /// watchlist.
    ///
    /// [`QuoteAlert::instrument_type`]: crate::types::quote_alert::QuoteAlert::instrument_type
    #[serde(default)]
    pub instrument_type: Option<String>,
}

/// A pairs watchlist: named equations rather than plain symbols.
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PairsWatchlist {
    /// The list's name, which is also how it is addressed in a URL.
    pub name: String,
    /// The pair equations on it.
    ///
    /// `Value` rather than a modelled type: the venue's schema types this
    /// `object` with **no properties at all**, so there is nothing to model
    /// against. Anything decodes, which is what keeps a list from being
    /// dropped over a field nobody has documented.
    #[serde(default)]
    pub pairs_equations: Vec<serde_json::Value>,
    /// Where it sorts among its siblings.
    #[serde(default)]
    pub order_index: Option<i32>,
}

/// A watchlist to create or replace.
///
/// Separate from [`Watchlist`] because the create body is not the read shape:
/// it has no `cms-id`, and sending one as `null` is a different request from
/// not sending it.
#[derive(DebugPretty, DisplaySimple, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct NewWatchlist {
    /// The list's name.
    pub name: String,
    /// The instruments on it.
    pub watchlist_entries: Vec<WatchlistEntry>,
    /// The group it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    /// Where it sorts among its siblings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_index: Option<i32>,
}

impl NewWatchlist {
    /// A list called `name` holding `symbols`.
    pub fn new(name: impl Into<String>, symbols: &[impl AsRef<str>]) -> Self {
        Self {
            name: name.into(),
            watchlist_entries: symbols
                .iter()
                .map(|symbol| WatchlistEntry {
                    symbol: Symbol(symbol.as_ref().to_owned()),
                    instrument_type: None,
                })
                .collect(),
            group_name: None,
            order_index: None,
        }
    }

    /// Adds an entry that names its instrument type.
    #[must_use]
    pub fn with_entry(
        mut self,
        symbol: impl Into<Symbol>,
        instrument_type: Option<String>,
    ) -> Self {
        self.watchlist_entries.push(WatchlistEntry {
            symbol: symbol.into(),
            instrument_type,
        });
        self
    }

    /// Which group it belongs to.
    #[must_use]
    pub fn with_group_name(mut self, group_name: impl Into<String>) -> Self {
        self.group_name = Some(group_name.into());
        self
    }

    /// Where it sorts.
    #[must_use]
    pub fn with_order_index(mut self, order_index: i32) -> Self {
        self.order_index = Some(order_index);
        self
    }

    /// Fails when the list cannot be what the venue accepts.
    ///
    /// Local checks, so [`crate::TastyTradeError::Precondition`] and not
    /// retryable. A blank name matters more than it looks: the name is the
    /// URL segment a later replace or delete addresses, and a list nobody can
    /// name is a list nobody can remove.
    pub(crate) fn validate(&self) -> crate::TastyResult<()> {
        if self.name.trim().is_empty() {
            return Err(crate::TastyTradeError::Precondition(
                "a watchlist needs a name, and this one is blank; the name is also \
                 how the list is addressed for replacement and deletion"
                    .to_string(),
            ));
        }

        for (index, entry) in self.watchlist_entries.iter().enumerate() {
            if entry.symbol.0.trim().is_empty() {
                return Err(crate::TastyTradeError::Precondition(format!(
                    "watchlist entry {index} has a blank symbol"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_public_watchlist_decodes() {
        let frame = r#"{
            "name": "High Options Volume",
            "group-name": "tastytrade",
            "order-index": 3,
            "cms-id": "blt-high-options-volume",
            "watchlist-entries": [
                {"symbol": "AAPL", "instrument-type": "Equity"},
                {"symbol": "/ES", "instrument-type": "Future"}
            ]
        }"#;

        let watchlist: Watchlist = serde_json::from_str(frame).expect("a watchlist decodes");

        assert_eq!(watchlist.name, "High Options Volume");
        assert_eq!(watchlist.watchlist_entries.len(), 2);
        assert_eq!(watchlist.watchlist_entries[1].symbol.0, "/ES");
        assert_eq!(watchlist.order_index, Some(3));
    }

    /// An emptied list is still a list, and the notification saying so is the
    /// one a caller most needs.
    #[test]
    fn a_watchlist_with_no_entries_decodes() {
        let watchlist: Watchlist =
            serde_json::from_str(r#"{"name":"Emptied"}"#).expect("a bare name is a watchlist");

        assert_eq!(watchlist.name, "Emptied");
        assert!(watchlist.watchlist_entries.is_empty());
        assert!(watchlist.group_name.is_none());
    }

    /// An entry without an instrument type is still an entry: the symbol is
    /// what the venue marks required.
    #[test]
    fn an_entry_without_an_instrument_type_decodes() {
        let watchlist: Watchlist =
            serde_json::from_str(r#"{"name":"L","watchlist-entries":[{"symbol":"SPY"}]}"#)
                .expect("a symbol is enough");

        assert_eq!(watchlist.watchlist_entries[0].symbol.0, "SPY");
        assert!(watchlist.watchlist_entries[0].instrument_type.is_none());
    }
}
