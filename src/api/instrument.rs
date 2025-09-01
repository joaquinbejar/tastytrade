/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 9/3/25
******************************************************************************/
use crate::api::base::{Items, Paginated};
use crate::types::instrument::{
    CompactOptionChain, Cryptocurrency, EquityInstrument, EquityInstrumentInfo, EquityOption,
    FutureOption, FutureOptionProduct, FutureProduct, NestedOptionChain, QuantityDecimalPrecision,
    Warrant,
};
use crate::{AsSymbol, TastyResult, TastyTrade};

impl TastyTrade {
    pub async fn get_equity_info(
        &self,
        symbol: impl AsSymbol,
    ) -> TastyResult<EquityInstrumentInfo> {
        self.get(format!("/instruments/equities/{}", symbol.as_symbol().0))
            .await
    }

    pub async fn list_equities(
        &self,
        symbols: &[impl AsSymbol],
    ) -> TastyResult<Vec<EquityInstrument>> {
        let mut query = Vec::<(&str, String)>::new();
        for symbol in symbols {
            let symbol_str = symbol.as_symbol().0.clone();
            query.push(("symbol[]", symbol_str));
        }

        let query_refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp: Items<EquityInstrument> = self
            .get_with_query("/instruments/equities", &query_refs)
            .await?;
        Ok(resp.items)
    }

    pub async fn list_active_equities(
        &self,
        page_offset: usize,
    ) -> TastyResult<Paginated<EquityInstrument>> {
        let page_offset_str = page_offset.to_string();
        let query = vec![
            ("per-page", "1000"),
            ("page-offset", page_offset_str.as_str()),
        ];

        self.get_with_query::<Items<EquityInstrument>, _, _>("/instruments/equities/active", &query)
            .await
    }

    pub async fn get_equity(&self, symbol: impl AsSymbol) -> TastyResult<EquityInstrument> {
        self.get(format!("/instruments/equities/{}", symbol.as_symbol().0))
            .await
    }

    pub async fn list_option_chains(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<Vec<EquityOption>> {
        let resp: Items<EquityOption> = self
            .get(format!(
                "/option-chains/{}",
                underlying_symbol.as_symbol().0
            ))
            .await?;
        Ok(resp.items)
    }

    pub async fn get_compact_option_chain(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<CompactOptionChain> {
        self.get(format!(
            "/option-chains/{}/compact",
            underlying_symbol.as_symbol().0
        ))
        .await
    }

    pub async fn list_nested_option_chains(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<Vec<NestedOptionChain>> {
        let resp: Items<NestedOptionChain> = self
            .get(format!(
                "/option-chains/{}/nested",
                underlying_symbol.as_symbol().0
            ))
            .await?;
        Ok(resp.items)
    }

    pub async fn list_equity_options(
        &self,
        symbols: &[impl AsSymbol],
        active: Option<bool>,
    ) -> TastyResult<Vec<EquityOption>> {
        let mut query = Vec::new();

        let mut symbol_strings = Vec::new();

        for symbol in symbols {
            symbol_strings.push(symbol.as_symbol().0.clone());
        }

        for symbol_str in &symbol_strings {
            query.push(("symbol[]", symbol_str.as_str()));
        }

        if let Some(active_val) = active {
            query.push(("active", if active_val { "true" } else { "false" }));
        }

        let resp: Items<EquityOption> = self
            .get_with_query("/instruments/equity-options", &query)
            .await?;
        Ok(resp.items)
    }

    /// List all equity options with pagination support
    pub async fn list_all_equity_options(
        &self,
        page_offset: usize,
        active: Option<bool>,
    ) -> TastyResult<Paginated<EquityOption>> {
        let page_offset_str = page_offset.to_string();
        let mut query = vec![
            ("per-page", "1000"),
            ("page-offset", page_offset_str.as_str()),
        ];

        if let Some(active_val) = active {
            query.push(("active", if active_val { "true" } else { "false" }));
        }

        self.get_with_query::<Items<EquityOption>, _, _>("/instruments/equity-options", &query)
            .await
    }

    pub async fn get_equity_option(&self, symbol: impl AsSymbol) -> TastyResult<EquityOption> {
        self.get(format!(
            "/instruments/equity-options/{}",
            symbol.as_symbol().0
        ))
        .await
    }

    pub async fn list_futures(
        &self,
        symbols: Option<&[impl AsSymbol]>,
        product_code: Option<&str>,
    ) -> TastyResult<Vec<crate::types::instrument::Future>> {
        let mut query = Vec::new();

        let mut symbol_strings = Vec::new();

        if let Some(symbols) = symbols {
            for symbol in symbols {
                symbol_strings.push(symbol.as_symbol().0.clone());
            }

            for symbol_str in &symbol_strings {
                query.push(("symbol[]", symbol_str.as_str()));
            }
        }

        if let Some(code) = product_code {
            query.push(("product-code", code));
        }

        let resp: Items<crate::types::instrument::Future> =
            self.get_with_query("/instruments/futures", &query).await?;
        Ok(resp.items)
    }

    pub async fn get_future(
        &self,
        symbol: impl AsSymbol,
    ) -> TastyResult<crate::types::instrument::Future> {
        self.get(format!("/instruments/futures/{}", symbol.as_symbol().0))
            .await
    }

    pub async fn list_future_products(&self) -> TastyResult<Vec<FutureProduct>> {
        let resp: Items<FutureProduct> = self.get("/instruments/future-products").await?;
        Ok(resp.items)
    }

    pub async fn get_future_product(
        &self,
        exchange: &str,
        code: &str,
    ) -> TastyResult<FutureProduct> {
        self.get(format!(
            "/instruments/future-products/{}/{}",
            exchange, code
        ))
        .await
    }

    pub async fn list_future_option_products(&self) -> TastyResult<Vec<FutureOptionProduct>> {
        let resp: Items<FutureOptionProduct> =
            self.get("/instruments/future-option-products").await?;
        Ok(resp.items)
    }

    pub async fn get_future_option_product_by_exchange(
        &self,
        exchange: &str,
        root_symbol: &str,
    ) -> TastyResult<FutureOptionProduct> {
        self.get(format!(
            "/instruments/future-option-products/{}/{}",
            exchange, root_symbol
        ))
        .await
    }

    pub async fn get_future_option_product(
        &self,
        root_symbol: &str,
    ) -> TastyResult<FutureOptionProduct> {
        self.get(format!(
            "/instruments/future-option-products/{}",
            root_symbol
        ))
        .await
    }

    pub async fn list_futures_option_chains(
        &self,
        product_code: &str,
    ) -> TastyResult<Vec<FutureOption>> {
        let resp: Items<FutureOption> = self
            .get(format!("/futures-option-chains/{}", product_code))
            .await?;
        Ok(resp.items)
    }

    pub async fn list_nested_futures_option_chains(
        &self,
        product_code: &str,
    ) -> TastyResult<Vec<NestedOptionChain>> {
        let resp: Items<NestedOptionChain> = self
            .get(format!("/futures-option-chains/{}/nested", product_code))
            .await?;
        Ok(resp.items)
    }

    pub async fn list_future_options(
        &self,
        symbols: &[impl AsSymbol],
    ) -> TastyResult<Vec<FutureOption>> {
        let mut query = Vec::new();
        let mut symbol_strings = Vec::new();

        for symbol in symbols {
            symbol_strings.push(symbol.as_symbol().0.clone());
        }

        for symbol_str in &symbol_strings {
            query.push(("symbol[]", symbol_str.as_str()));
        }

        let resp: Items<FutureOption> = self
            .get_with_query("/instruments/future-options", &query)
            .await?;
        Ok(resp.items)
    }

    pub async fn get_future_option(&self, symbol: impl AsSymbol) -> TastyResult<FutureOption> {
        self.get(format!(
            "/instruments/future-options/{}",
            symbol.as_symbol().0
        ))
        .await
    }

    pub async fn list_cryptocurrencies(&self) -> TastyResult<Vec<Cryptocurrency>> {
        let resp: Items<Cryptocurrency> = self.get("/instruments/cryptocurrencies").await?;
        Ok(resp.items)
    }

    pub async fn get_cryptocurrency(&self, symbol: impl AsSymbol) -> TastyResult<Cryptocurrency> {
        self.get(format!(
            "/instruments/cryptocurrencies/{}",
            symbol.as_symbol().0
        ))
        .await
    }

    pub async fn list_warrants(
        &self,
        symbols: Option<&[impl AsSymbol]>,
    ) -> TastyResult<Vec<Warrant>> {
        let mut query = Vec::new();
        let mut symbol_strings = Vec::new();

        if let Some(symbols) = symbols {
            for symbol in symbols {
                symbol_strings.push(symbol.as_symbol().0.clone());
            }

            for symbol_str in &symbol_strings {
                query.push(("symbol[]", symbol_str.as_str()));
            }
        }

        let resp: Items<Warrant> = self.get_with_query("/instruments/warrants", &query).await?;
        Ok(resp.items)
    }

    pub async fn get_warrant(&self, symbol: impl AsSymbol) -> TastyResult<Warrant> {
        self.get(format!("/instruments/warrants/{}", symbol.as_symbol().0))
            .await
    }

    pub async fn list_quantity_decimal_precisions(
        &self,
    ) -> TastyResult<Vec<QuantityDecimalPrecision>> {
        let resp: Items<QuantityDecimalPrecision> =
            self.get("/instruments/quantity-decimal-precisions").await?;
        Ok(resp.items)
    }
}
