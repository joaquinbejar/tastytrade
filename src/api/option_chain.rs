use super::{base::Items, quote_streaming::DxFeedSymbol, url::encode_path_segment};
use crate::api::base::TastyResult;
use crate::types::wire::{ExpirationType, SettlementType};
use crate::{AsSymbol, Symbol, TastyTrade};
use chrono::NaiveDate;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

impl TastyTrade {
    /// The nested option chain for one underlying.
    ///
    /// # Errors
    ///
    /// Fails when the venue returns no chain for the symbol, which is an
    /// error rather than a panic — it used to be the latter.
    pub async fn nested_option_chain_for(
        &self,
        symbol: impl Into<Symbol>,
    ) -> TastyResult<NestedOptionChain> {
        let symbol = symbol.into();
        let resp: Items<NestedOptionChain> = self
            .get(format!(
                "/option-chains/{}/nested",
                encode_path_segment(&symbol.0)
            ))
            .await?;
        resp.into_items()?.into_iter().next().ok_or_else(|| {
            crate::TastyTradeError::Unknown(format!(
                "No nested option chain found for symbol {}",
                symbol.0
            ))
        })
    }

    /// The flat option chain for one underlying.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn option_chain_for(
        &self,
        symbol: impl Into<Symbol>,
    ) -> TastyResult<Vec<OptionChain>> {
        let resp: Items<OptionChain> = self
            .get(format!(
                "/option-chains/{}",
                encode_path_segment(&symbol.into().0)
            ))
            .await?;
        resp.into_items()
    }

    /// Looks up the streaming name for one option symbol.
    ///
    /// # Errors
    ///
    /// Fails when the option is unknown to the venue.
    pub async fn get_option_info(&self, symbol: impl AsSymbol) -> TastyResult<OptionInfo> {
        self.get(format!(
            "/instruments/equity-options/{}",
            encode_path_segment(&symbol.as_symbol().0)
        ))
        .await
    }
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// What the streaming feed calls one option.
pub struct OptionInfo {
    /// The symbol to subscribe with, which is not always the instrument
    /// symbol.
    pub streamer_symbol: DxFeedSymbol,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// An option chain grouped by expiration and then by strike.
pub struct NestedOptionChain {
    /// The instrument the options are written on.
    pub underlying_symbol: Symbol,
    /// The option root, which differs from the underlying after a corporate
    /// action.
    pub root_symbol: Symbol,
    /// Standard or non-standard. Still text: no captured payload in this
    /// repository shows its value set, so it is not modelled as an enum.
    pub option_chain_type: String,
    /// Shares delivered per contract. Not always 100 after a split.
    pub shares_per_contract: u64,
    /// The expirations, each with its strikes.
    pub expirations: Vec<Expiration>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// One expiration and the strikes listed against it.
pub struct Expiration {
    /// Regular, weekly, quarterly or end-of-month.
    pub expiration_type: ExpirationType,
    /// The calendar date, with no time and no zone: an expiration is a day on
    /// an exchange calendar, not an instant.
    #[serde(with = "crate::types::wire::date")]
    pub expiration_date: NaiveDate,
    /// Days remaining, as the venue counts them.
    pub days_to_expiration: u64,
    /// Morning or afternoon settlement.
    pub settlement_type: SettlementType,
    /// The strikes at this expiration.
    pub strikes: Vec<Strike>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// One strike, with the two contracts written at it.
pub struct Strike {
    /// The strike, at full precision.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    /// The call contract's symbol.
    pub call: Symbol,
    /// The put contract's symbol.
    pub put: Symbol,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// One option from a flat chain.
pub struct OptionChain {
    /// The instrument the option is written on.
    pub underlying_symbol: Symbol,
    /// The strike, at full precision.
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub strike_price: Decimal,
    /// Everything else the venue sent, kept rather than dropped.
    ///
    /// This endpoint returns a wide and undocumented set of fields that
    /// changes without notice, so they are preserved verbatim instead of
    /// being modelled and going stale.
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
        assert_eq!(expiration.expiration_type.to_string(), "Regular");
        assert_eq!(
            expiration.expiration_date,
            NaiveDate::from_ymd_opt(2024, 9, 20).unwrap()
        );
        assert_eq!(expiration.days_to_expiration, 30);
        assert_eq!(expiration.settlement_type.to_string(), "PM");
        assert_eq!(expiration.strikes.len(), 1);
        assert_eq!(
            expiration.strikes[0].strike_price,
            Decimal::from_str("150.00").unwrap()
        );
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
        assert_eq!(
            chain.extra.get("extra-field").unwrap().as_str().unwrap(),
            "extra-value"
        );
        assert_eq!(
            chain.extra.get("another-field").unwrap().as_i64().unwrap(),
            42
        );
    }

    #[test]
    fn test_debug_implementations() {
        let option_info = OptionInfo {
            streamer_symbol: DxFeedSymbol("TEST".to_string()),
        };
        let debug_str = format!("{:?}", option_info);
        assert!(debug_str.contains("TEST"));

        let strike = Strike {
            strike_price: Decimal::from_str("100.00").unwrap(),
            call: Symbol::from("CALL"),
            put: Symbol::from("PUT"),
        };
        let debug_str = format!("{:?}", strike);
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
        assert_eq!(expiration.expiration_type.to_string(), "Weekly");
        assert_eq!(expiration.strikes.len(), 3);

        // Test first strike
        assert_eq!(
            expiration.strikes[0].strike_price,
            Decimal::from_str("145.00").unwrap()
        );
        assert_eq!(expiration.strikes[0].call.0, "AAPL240927C00145000");

        // Test middle strike
        assert_eq!(
            expiration.strikes[1].strike_price,
            Decimal::from_str("150.00").unwrap()
        );
        assert_eq!(expiration.strikes[1].put.0, "AAPL240927P00150000");

        // Test last strike
        assert_eq!(
            expiration.strikes[2].strike_price,
            Decimal::from_str("155.00").unwrap()
        );
    }
}
