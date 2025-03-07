use crate::TastyTrade;
use crate::api::base::TastyApiResponse;
use crate::{AsSymbol, InstrumentType, Symbol, TastyResult};
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteStreamerTokens {
    pub token: String,
    #[serde(rename = "dxlink-url")]
    pub streamer_url: String,
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
    ) -> TastyResult<DxFeedSymbol> {
        use InstrumentType::*;
        let sym = match instrument_type {
            Equity => self.get_equity_info(symbol).await?.streamer_symbol,
            EquityOption => self.get_option_info(symbol).await?.streamer_symbol,
            _ => unimplemented!(),
        };
        Ok(sym)
    }
}
