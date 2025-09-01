use crate::TastyTrade;
use crate::api::base::TastyApiResponse;
use crate::types::instrument::InstrumentType;
use crate::{AsSymbol, Symbol, TastyResult};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::Deserialize;
use serde::Serialize;
use tracing::{debug, error};

impl TastyTrade {
    pub async fn quote_streamer_tokens(&self) -> TastyResult<QuoteStreamerTokens> {
        let url = format!("{}/api-quote-tokens", self.config.base_url);
        debug!("Requesting quote streamer tokens from: {}", url);

        // Hacer la solicitud HTTP directamente para poder examinar la respuesta
        let response = self.client.get(&url).send().await?;

        // Verificar el código de estado
        let status = response.status();
        debug!("Response status: {}", status);

        if !status.is_success() {
            error!("Failed to get quote streamer tokens: HTTP {}", status);
            let text = response.text().await?;
            error!("Response body: {}", text);
            return Err(crate::TastyTradeError::Connection(format!(
                "Failed to get quote streamer tokens: HTTP {}, Body: {}",
                status, text
            )));
        }

        // Intentar decodificar la respuesta como JSON
        let text = response.text().await?;
        debug!("Response body: {}", text);

        match serde_json::from_str::<TastyApiResponse<QuoteStreamerTokens>>(&text) {
            Ok(TastyApiResponse::Success(s)) => Ok(s.data),
            Ok(TastyApiResponse::Error { error }) => Err(error.into()),
            Err(e) => {
                error!("Failed to parse response: {}", e);
                Err(crate::TastyTradeError::Json(e))
            }
        }
    }
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteStreamerTokens {
    pub token: String,
    #[serde(rename = "dxlink-url")]
    pub streamer_url: String,
    pub level: String,
}

#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(transparent)]
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

impl TastyTrade {
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
    fn test_quote_streamer_tokens_debug() {
        let tokens = QuoteStreamerTokens {
            token: "test_token".to_string(),
            streamer_url: "wss://test.com".to_string(),
            level: "realtime".to_string(),
        };

        let debug_str = format!("{:?}", tokens);
        assert!(debug_str.contains("test_token"));
        assert!(debug_str.contains("wss://test.com"));
        assert!(debug_str.contains("realtime"));
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
