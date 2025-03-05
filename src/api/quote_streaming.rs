use serde::Deserialize;
use serde::Serialize;

use crate::Result;
use crate::TastyTrade;

use super::order::AsSymbol;
use super::order::InstrumentType;
use super::order::Symbol;

impl TastyTrade {
    pub async fn quote_streamer_tokens(&self) -> Result<QuoteStreamerTokens> {
        self.get("/quote-streamer-tokens").await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteStreamerTokens {
    pub token: String,
    pub streamer_url: String,
    pub websocket_url: String,
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    ) -> Result<DxFeedSymbol> {
        use InstrumentType::*;
        let sym = match instrument_type {
            Equity => self.get_equity_info(symbol).await?.streamer_symbol,
            EquityOption => self.get_option_info(symbol).await?.streamer_symbol,
            _ => unimplemented!(),
        };
        Ok(sym)
    }
}
