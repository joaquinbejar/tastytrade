use crate::TastyResult;
use crate::TastyTrade;
use crate::api::quote_streaming::DxFeedSymbol;
use serde::Deserialize;

use super::order::AsSymbol;
use super::order::Symbol;

impl TastyTrade {
    pub async fn get_equity_info(
        &self,
        symbol: impl AsSymbol,
    ) -> TastyResult<EquityInstrumentInfo> {
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
