use serde::Deserialize;

use crate::Result;
use crate::TastyTrade;

use super::order::AsSymbol;
use super::order::Symbol;
use super::quote_streaming::DxFeedSymbol;

impl TastyTrade {
    pub async fn get_equity_info(&self, symbol: impl AsSymbol) -> Result<EquityInstrumentInfo> {
        self.get(format!("/instruments/equities/{}", symbol.as_symbol().0))
            .await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityInstrumentInfo {
    pub symbol: Symbol,
    pub streamer_symbol: DxFeedSymbol,
}
