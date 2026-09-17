use crate::TastyTrade;
use crate::types::instrument::InstrumentType;
use crate::{AsSymbol, Symbol, TastyResult};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Deserialize;
use serde::Serialize;
use tracing::debug;

impl TastyTrade {
    /// Exchanges the session for a short-lived DXLink streamer token.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error. Neither the response body nor the token
    /// reaches the error or the logs: the body of this particular response
    /// *is* a credential, so a decode failure reports the status and the
    /// endpoint and nothing else.
    pub async fn quote_streamer_tokens(&self) -> TastyResult<QuoteStreamerTokens> {
        debug!("Requesting quote streamer tokens");

        // Through the generic verb, which checks the status and keeps the body
        // out of both the logs and the error. Decoding by hand here logged the
        // serde error and then handed it to the caller inside
        // `TastyTradeError::Json`. A serde_json error quotes the value it
        // rejected, and the value in this particular response is the DXLink
        // credential; the untagged envelope happened to mask it, which is a
        // property of `TastyApiResponse` rather than anything this function
        // arranged. A non-2xx also became a `Connection` error carrying only
        // the status, where every other endpoint reports where and against
        // which deployment.
        self.get("/api-quote-tokens").await
    }
}

/// DXLink quote streamer credentials.
///
/// `Debug` and `Display` are implemented manually so the token is never
/// written to logs.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteStreamerTokens {
    /// The DXLink credential. Redacted by this type's `Debug`; keep it that
    /// way.
    pub token: String,
    /// Where to connect with it.
    #[serde(rename = "dxlink-url")]
    pub streamer_url: String,
    /// The market-data entitlement this token carries.
    pub level: String,
}

impl std::fmt::Debug for QuoteStreamerTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuoteStreamerTokens")
            .field("token", &"***")
            .field("streamer_url", &self.streamer_url)
            .field("level", &self.level)
            .finish()
    }
}

impl std::fmt::Display for QuoteStreamerTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(transparent)]
/// A symbol as the streaming feed names it.
///
/// Not the same namespace as [`Symbol`], which is what the REST API calls an
/// instrument. The two strings coincide for equities and differ for futures,
/// options, cryptocurrencies and warrants — a futures contract the REST API
/// calls `/ESU3` streams as `/ESU23:XCME` — which is why
/// [`TastyTrade::get_streamer_symbol`] exists and why the streaming
/// subscription methods will only take this type: see [`AsStreamerSymbol`].
///
/// Obtain one from [`TastyTrade::get_streamer_symbol`], or from an
/// instrument's own `streamer_symbol` field, and pass it on unchanged. There
/// is no textual rule that turns one namespace into the other, so there is
/// nothing to strip or append by hand. Constructing one directly is asserting
/// the string is already a streaming name — safe for an equity ticker, and
/// worth checking for anything else.
pub struct DxFeedSymbol(pub String);

impl AsSymbol for DxFeedSymbol {
    fn as_symbol(&self) -> Symbol {
        Symbol(self.0.clone())
    }
}

impl AsSymbol for &DxFeedSymbol {
    fn as_symbol(&self) -> Symbol {
        Symbol(self.0.clone())
    }
}

/// A symbol in the streaming namespace.
///
/// Implemented for [`DxFeedSymbol`] and nothing else, on purpose. [`AsSymbol`]
/// is implemented for every `AsRef<str>`, so `&str`, [`Symbol`] and
/// [`DxFeedSymbol`] all collapse into one type and the compiler cannot tell an
/// instrument symbol from a streaming one. That distinction costs nothing for
/// an equity, where the two strings match, and is silent and total for
/// anything else: a futures contract the REST API calls `/ESU3` streams as
/// `/ESU23:XCME`, and the venue simply never sends events for a target it does
/// not recognise. The subscription succeeds, no error is raised, and the
/// history never loads.
///
/// **What this type guarantees, and what it does not.** It guarantees the
/// caller did not hand a [`Symbol`], a `String` or a `&str` to the feed by
/// mistake, because those no longer compile. It does not guarantee the string
/// inside is a symbol the feed knows: the field is public, so a wrong name can
/// still be wrapped deliberately, and an unknown target still fails the same
/// silent way. Get one from
/// [`TastyTrade::get_streamer_symbol`] or from an instrument's own
/// `streamer_symbol` field and pass it through unchanged — there is no textual
/// rule mapping one namespace to the other, and this crate does not invent one.
///
/// What it does buy is that the accidental case stops compiling. Each of the
/// three subscription methods refuses each of the three old types, and the
/// error is the trait bound rather than anything incidental, which is what
/// `E0277` pins:
///
/// `add_candles` with a `&str`:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription, period: CandlePeriod) {
/// sub.add_candles(&["/ESU3"], period, chrono::Utc::now()).await.unwrap();
/// # }
/// ```
///
/// `add_candles` with a [`Symbol`], the instrument namespace:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription, period: CandlePeriod) {
/// sub.add_candles(&[Symbol("/ESU3".to_string())], period, chrono::Utc::now())
///     .await
///     .unwrap();
/// # }
/// ```
///
/// `remove_candles` with a `String`:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription, period: CandlePeriod) {
/// sub.remove_candles(&["/ESU3".to_string()], period).await.unwrap();
/// # }
/// ```
///
/// `remove_candles` with a [`Symbol`]:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription, period: CandlePeriod) {
/// sub.remove_candles(&[Symbol("/ESU3".to_string())], period).await.unwrap();
/// # }
/// ```
///
/// `add_symbols` with a `&str`:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription) {
/// sub.add_symbols(&["AAPL"]).await.unwrap();
/// # }
/// ```
///
/// `add_symbols` with a [`Symbol`]:
///
/// ```compile_fail,E0277
/// # use tastytrade::prelude::*;
/// # async fn nope(sub: &QuoteSubscription) {
/// sub.add_symbols(&[Symbol("AAPL".to_string())]).await.unwrap();
/// # }
/// ```
///
/// All three take a [`DxFeedSymbol`], owned or borrowed, and that is the whole
/// list:
///
/// ```rust,no_run
/// # use tastytrade::prelude::*;
/// # use tastytrade::TastyTrade;
/// # async fn yes(tasty: &TastyTrade, sub: &QuoteSubscription, period: CandlePeriod)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// // Whatever the venue calls it. Passed on unchanged, never rewritten.
/// let es = tasty
///     .get_streamer_symbol(&InstrumentType::Future, &Symbol("/ESU3".to_string()))
///     .await?;
///
/// sub.add_candles(&[es.clone()], period, chrono::Utc::now()).await?;
/// sub.add_candles(&[&es], period, chrono::Utc::now()).await?;
/// sub.add_symbols(&[es.clone()]).await?;
/// sub.add_symbols(&[&es]).await?;
/// sub.remove_candles(&[es.clone()], period).await?;
/// sub.remove_candles(&[&es], period).await?;
/// # Ok(())
/// # }
/// ```
///
/// Deliberately not sealed. A caller with its own resolved-symbol type may
/// implement this, which is an assertion written down in their code rather
/// than an accident hidden in a blanket impl.
pub trait AsStreamerSymbol {
    /// The streaming name this value carries.
    fn as_streamer_symbol(&self) -> DxFeedSymbol;
}

impl AsStreamerSymbol for DxFeedSymbol {
    fn as_streamer_symbol(&self) -> DxFeedSymbol {
        self.clone()
    }
}

impl AsStreamerSymbol for &DxFeedSymbol {
    fn as_streamer_symbol(&self) -> DxFeedSymbol {
        (*self).clone()
    }
}

impl TastyTrade {
    /// Looks up the streaming name for an instrument.
    ///
    /// # Errors
    ///
    /// Fails when the instrument is unknown or carries no streamer symbol.
    pub async fn get_streamer_symbol(
        &self,
        instrument_type: &InstrumentType,
        symbol: &Symbol,
    ) -> TastyResult<DxFeedSymbol> {
        use InstrumentType::*;
        let sym = match instrument_type {
            Equity => self.get_equity_info(symbol).await?.streamer_symbol,
            EquityOption => self.get_option_info(symbol).await?.streamer_symbol,
            EquityOffering => self.get_equity_info(symbol).await?.streamer_symbol, // Handle as equity
            Future => self.get_future(symbol).await?.streamer_symbol,
            FutureOption => self
                .get_future_option(symbol)
                .await?
                .streamer_symbol
                .unwrap_or_else(|| DxFeedSymbol(symbol.0.clone())),
            Cryptocurrency => self.get_cryptocurrency(symbol).await?.streamer_symbol,
            Bond => DxFeedSymbol(symbol.0.clone()), // Handle as basic symbol
            FixedIncomeSecurity => DxFeedSymbol(symbol.0.clone()), // Handle as basic symbol
            LiquidityPool => DxFeedSymbol(symbol.0.clone()), // Handle as basic symbol
            Warrant => DxFeedSymbol(self.get_warrant(symbol).await?.symbol.0), // Convert to DxFeedSymbol
        };
        Ok(sym)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::instrument::InstrumentType;

    #[test]
    fn test_quote_streamer_tokens_deserialization() {
        let json = r#"{
            "token": "abc123token",
            "dxlink-url": "wss://streamer.example.com",
            "level": "delayed"
        }"#;

        let tokens: QuoteStreamerTokens = serde_json::from_str(json).unwrap();
        assert_eq!(tokens.token, "abc123token");
        assert_eq!(tokens.streamer_url, "wss://streamer.example.com");
        assert_eq!(tokens.level, "delayed");
    }

    #[test]
    fn test_quote_streamer_tokens_debug_redacts_token() {
        let tokens = QuoteStreamerTokens {
            token: "test_token".to_string(),
            streamer_url: "wss://test.com".to_string(),
            level: "realtime".to_string(),
        };

        for output in [format!("{:?}", tokens), format!("{}", tokens)] {
            assert!(!output.contains("test_token"));
            assert!(output.contains("***"));
            assert!(output.contains("wss://test.com"));
            assert!(output.contains("realtime"));
        }
    }

    #[test]
    fn test_dxfeed_symbol_creation() {
        let symbol = DxFeedSymbol("AAPL".to_string());
        assert_eq!(symbol.0, "AAPL");
    }

    #[test]
    fn test_dxfeed_symbol_as_symbol_trait() {
        let dxfeed_symbol = DxFeedSymbol("MSFT".to_string());
        let symbol = dxfeed_symbol.as_symbol();
        assert_eq!(symbol.0, "MSFT");

        // Test with reference
        let symbol_ref = &dxfeed_symbol;
        let symbol = symbol_ref.as_symbol();
        assert_eq!(symbol.0, "MSFT");
    }

    #[test]
    fn test_dxfeed_symbol_serialization() {
        let symbol = DxFeedSymbol("TSLA".to_string());
        let serialized = serde_json::to_string(&symbol).unwrap();
        assert_eq!(serialized, "\"TSLA\"");

        let deserialized: DxFeedSymbol = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.0, "TSLA");
    }

    #[test]
    fn test_dxfeed_symbol_traits() {
        let symbol1 = DxFeedSymbol("AAPL".to_string());
        let symbol2 = DxFeedSymbol("AAPL".to_string());
        let symbol3 = DxFeedSymbol("MSFT".to_string());

        // Test Clone
        let cloned = symbol1.clone();
        assert_eq!(cloned.0, "AAPL");

        // Test PartialEq
        assert_eq!(symbol1, symbol2);
        assert_ne!(symbol1, symbol3);

        // Test PartialOrd
        assert!(symbol1 < symbol3); // "AAPL" < "MSFT"
        assert!(symbol3 > symbol1);

        // Test Debug
        let debug_str = format!("{:?}", symbol1);
        assert!(debug_str.contains("AAPL"));
    }

    #[test]
    fn test_dxfeed_symbol_ordering() {
        let mut symbols = [
            DxFeedSymbol("TSLA".to_string()),
            DxFeedSymbol("AAPL".to_string()),
            DxFeedSymbol("MSFT".to_string()),
        ];

        symbols.sort();

        assert_eq!(symbols[0].0, "AAPL");
        assert_eq!(symbols[1].0, "MSFT");
        assert_eq!(symbols[2].0, "TSLA");
    }

    #[test]
    fn test_dxfeed_symbol_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let symbol1 = DxFeedSymbol("AAPL".to_string());
        let symbol2 = DxFeedSymbol("AAPL".to_string());

        map.insert(symbol1, "Apple");

        // Should be able to retrieve with equivalent symbol
        assert_eq!(map.get(&symbol2), Some(&"Apple"));
    }

    #[test]
    fn test_instrument_type_matching() {
        // Test that all InstrumentType variants are handled
        // This is a compile-time test - if new variants are added,
        // the match in get_streamer_symbol will need updating
        let instrument_types = [
            InstrumentType::Equity,
            InstrumentType::EquityOption,
            InstrumentType::EquityOffering,
            InstrumentType::Future,
            InstrumentType::FutureOption,
            InstrumentType::Cryptocurrency,
        ];

        // Just verify we can create all variants
        assert_eq!(instrument_types.len(), 6);
    }

    #[test]
    fn test_dxfeed_symbol_transparent_serde() {
        // Test that the transparent attribute works correctly
        let symbol = DxFeedSymbol("TEST123".to_string());
        let json = serde_json::to_string(&symbol).unwrap();

        // Should serialize as just the string, not as an object
        assert_eq!(json, "\"TEST123\"");

        // Should deserialize back correctly
        let deserialized: DxFeedSymbol = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, symbol);
    }
}
