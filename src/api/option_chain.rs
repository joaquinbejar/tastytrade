use super::{base::Items, quote_streaming::DxFeedSymbol};
use crate::api::base::TastyResult;
use crate::{AsSymbol, Symbol, TastyTrade};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use pretty_simple_display::{DebugPretty, DisplaySimple};

impl TastyTrade {
    pub async fn nested_option_chain_for(
        &self,
        symbol: impl Into<Symbol>,
    ) -> TastyResult<NestedOptionChain> {
        let mut resp: Items<NestedOptionChain> = self
            .get(format!("/option-chains/{}/nested", symbol.into().0))
            .await?;
        Ok(resp.items.remove(0))
    }

    pub async fn option_chain_for(
        &self,
        symbol: impl Into<Symbol>,
    ) -> TastyResult<Vec<OptionChain>> {
        let resp: Items<OptionChain> = self
            .get(format!("/option-chains/{}", symbol.into().0))
            .await?;
        Ok(resp.items)
    }

    pub async fn get_option_info(&self, symbol: impl AsSymbol) -> TastyResult<OptionInfo> {
        self.get(format!(
            "/instruments/equity-options/{}",
            symbol.as_symbol().0
        ))
        .await
    }
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionInfo {
    pub streamer_symbol: DxFeedSymbol,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NestedOptionChain {
    pub underlying_symbol: Symbol,
    pub root_symbol: Symbol,
    pub option_chain_type: String,
    pub shares_per_contract: u64,
    pub expirations: Vec<Expiration>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Expiration {
    pub expiration_type: String,
    pub expiration_date: String,
    pub days_to_expiration: u64,
    pub settlement_type: String,
    pub strikes: Vec<Strike>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Strike {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    pub call: Symbol,
    pub put: Symbol,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionChain {
    pub underlying_symbol: Symbol,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_option_info_deserialization() {
        let json = r#"{
            "streamer-symbol": "AAPL240920C00150000"
        }"#;
        
        let option_info: OptionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(option_info.streamer_symbol.0, "AAPL240920C00150000");
    }

    #[test]
    fn test_strike_deserialization() {
        let json = r#"{
            "strike-price": "150.00",
            "call": "AAPL240920C00150000",
            "put": "AAPL240920P00150000"
        }"#;
        
        let strike: Strike = serde_json::from_str(json).unwrap();
        assert_eq!(strike.strike_price, Decimal::from_str("150.00").unwrap());
        assert_eq!(strike.call.0, "AAPL240920C00150000");
        assert_eq!(strike.put.0, "AAPL240920P00150000");
    }

    #[test]
    fn test_expiration_deserialization() {
        let json = r#"{
            "expiration-type": "Regular",
            "expiration-date": "2024-09-20",
            "days-to-expiration": 30,
            "settlement-type": "PM",
            "strikes": [
                {
                    "strike-price": "150.00",
                    "call": "AAPL240920C00150000",
                    "put": "AAPL240920P00150000"
                }
            ]
        }"#;
        
        let expiration: Expiration = serde_json::from_str(json).unwrap();
        assert_eq!(expiration.expiration_type, "Regular");
        assert_eq!(expiration.expiration_date, "2024-09-20");
        assert_eq!(expiration.days_to_expiration, 30);
        assert_eq!(expiration.settlement_type, "PM");
        assert_eq!(expiration.strikes.len(), 1);
        assert_eq!(expiration.strikes[0].strike_price, Decimal::from_str("150.00").unwrap());
    }

    #[test]
    fn test_nested_option_chain_deserialization() {
        let json = r#"{
            "underlying-symbol": "AAPL",
            "root-symbol": "AAPL",
            "option-chain-type": "Standard",
            "shares-per-contract": 100,
            "expirations": [
                {
                    "expiration-type": "Regular",
                    "expiration-date": "2024-09-20",
                    "days-to-expiration": 30,
                    "settlement-type": "PM",
                    "strikes": []
                }
            ]
        }"#;
        
        let chain: NestedOptionChain = serde_json::from_str(json).unwrap();
        assert_eq!(chain.underlying_symbol.0, "AAPL");
        assert_eq!(chain.root_symbol.0, "AAPL");
        assert_eq!(chain.option_chain_type, "Standard");
        assert_eq!(chain.shares_per_contract, 100);
        assert_eq!(chain.expirations.len(), 1);
    }

    #[test]
    fn test_option_chain_deserialization() {
        let json = r#"{
            "underlying-symbol": "MSFT",
            "strike-price": "300.00",
            "extra-field": "extra-value",
            "another-field": 42
        }"#;
        
        let chain: OptionChain = serde_json::from_str(json).unwrap();
        assert_eq!(chain.underlying_symbol.0, "MSFT");
        assert_eq!(chain.strike_price, Decimal::from_str("300.00").unwrap());
        assert_eq!(chain.extra.len(), 2);
        assert_eq!(chain.extra.get("extra-field").unwrap().as_str().unwrap(), "extra-value");
        assert_eq!(chain.extra.get("another-field").unwrap().as_i64().unwrap(), 42);
    }

    #[test]
    fn test_debug_implementations() {
        let option_info = OptionInfo {
            streamer_symbol: DxFeedSymbol("TEST".to_string()),
        };
        let debug_str = format!("{:?}", option_info);
        assert!(debug_str.contains("OptionInfo"));
        assert!(debug_str.contains("TEST"));
        
        let strike = Strike {
            strike_price: Decimal::from_str("100.00").unwrap(),
            call: Symbol::from("CALL"),
            put: Symbol::from("PUT"),
        };
        let debug_str = format!("{:?}", strike);
        assert!(debug_str.contains("Strike"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_multiple_strikes_in_expiration() {
        let json = r#"{
            "expiration-type": "Weekly",
            "expiration-date": "2024-09-27",
            "days-to-expiration": 7,
            "settlement-type": "AM",
            "strikes": [
                {
                    "strike-price": "145.00",
                    "call": "AAPL240927C00145000",
                    "put": "AAPL240927P00145000"
                },
                {
                    "strike-price": "150.00",
                    "call": "AAPL240927C00150000",
                    "put": "AAPL240927P00150000"
                },
                {
                    "strike-price": "155.00",
                    "call": "AAPL240927C00155000",
                    "put": "AAPL240927P00155000"
                }
            ]
        }"#;
        
        let expiration: Expiration = serde_json::from_str(json).unwrap();
        assert_eq!(expiration.expiration_type, "Weekly");
        assert_eq!(expiration.strikes.len(), 3);
        
        // Test first strike
        assert_eq!(expiration.strikes[0].strike_price, Decimal::from_str("145.00").unwrap());
        assert_eq!(expiration.strikes[0].call.0, "AAPL240927C00145000");
        
        // Test middle strike
        assert_eq!(expiration.strikes[1].strike_price, Decimal::from_str("150.00").unwrap());
        assert_eq!(expiration.strikes[1].put.0, "AAPL240927P00150000");
        
        // Test last strike
        assert_eq!(expiration.strikes[2].strike_price, Decimal::from_str("155.00").unwrap());
    }
}
