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
    #[serde(default)]
    pub instrument_type: Option<String>,
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
