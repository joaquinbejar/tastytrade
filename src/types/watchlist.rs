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
    /// A raw [`serde_json::Value`] rather than a modelled type, and rather
    /// than a `Vec`: the venue's schema types this `object` with **no
    /// properties at all**, so there is nothing to model against and no reason
    /// to believe it is an array. Hard-coding the array shape meant that if the
    /// venue sends the object its own schema describes, every pairs list fails
    /// to decode and `Items<T>` drops it — the whole listing comes back empty
    /// with nothing said.
    ///
    /// [`PairsWatchlist::equations`] gives the array when there is one, so a
    /// caller that wants to iterate is not forced to match on the shape.
    #[serde(default)]
    pub pairs_equations: serde_json::Value,
    /// Where it sorts among its siblings.
    #[serde(default)]
    pub order_index: Option<i32>,
}

impl PairsWatchlist {
    /// The equations as a list, when the venue sent one.
    ///
    /// `None` when it sent something else — an object, or nothing at all.
    /// That is a real answer rather than an empty list: "the venue sent a
    /// shape this crate does not iterate" and "there are no equations" are
    /// different facts, and only one of them means the list is empty.
    pub fn equations(&self) -> Option<&[serde_json::Value]> {
        self.pairs_equations.as_array().map(Vec::as_slice)
    }
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
        validate_watchlist_name(&self.name)?;

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

/// Fails when a name cannot address a watchlist.
///
/// The destructive path takes a bare name rather than a [`NewWatchlist`], so
/// it never ran the body's validation: a blank argument was sent as
/// `DELETE /watchlists/` or an encoded blank segment, which is a request
/// against a route nobody meant to call, on the one method here that destroys
/// data irreversibly.
pub(crate) fn validate_watchlist_name(name: &str) -> crate::TastyResult<()> {
    if name.trim().is_empty() {
        return Err(crate::TastyTradeError::Precondition(
            "a watchlist needs a name, and this one is blank; the name is also \
             how the list is addressed for replacement and deletion"
                .to_string(),
        ));
    }
    Ok(())
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

    /// The pairs equations decode whatever shape arrives.
    ///
    /// The venue's schema types this `object` with no properties at all, so
    /// the array is the mock's shape rather than a promise. Hard-coding it
    /// meant that the object the schema actually describes failed the whole
    /// list, and `Items<T>` drops what fails — so the listing would come back
    /// empty with nothing said.
    #[test]
    fn a_pairs_list_survives_either_equation_shape() {
        let array: PairsWatchlist = serde_json::from_str(
            r#"{"name": "Pairs", "pairs-equations": [{"left": "AAPL", "right": "MSFT"}]}"#,
        )
        .expect("the array shape must decode");
        assert_eq!(array.equations().expect("an array").len(), 1);

        // The shape the published schema describes.
        let object: PairsWatchlist = serde_json::from_str(
            r#"{"name": "Pairs", "pairs-equations": {"AAPL/MSFT": {"ratio": 1}}}"#,
        )
        .expect("the object shape must decode too");
        assert!(
            object.equations().is_none(),
            "an object is not a list, and saying so beats returning an empty one"
        );
        assert!(object.pairs_equations.is_object(), "the value is kept");

        // And an absent field is neither.
        let bare: PairsWatchlist =
            serde_json::from_str(r#"{"name": "Pairs"}"#).expect("must decode");
        assert!(bare.equations().is_none());
    }

    /// The destructive path validates its own name.
    ///
    /// It takes a bare name rather than a body, so the body's validation never
    /// ran on it: a blank argument went out as `DELETE /watchlists/`.
    #[test]
    fn a_blank_name_cannot_address_a_watchlist() {
        for name in ["", "   ", "\t"] {
            let error = validate_watchlist_name(name).expect_err("a blank name is not a name");
            assert!(matches!(error, crate::TastyTradeError::Precondition(_)));
            assert!(!error.is_retryable(), "nothing was sent");
        }
        validate_watchlist_name("My List").expect("a real name");
    }
}
