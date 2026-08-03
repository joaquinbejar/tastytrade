/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 9/3/25
******************************************************************************/
use crate::api::base::{Items, Paginated};
use crate::api::query::{PageRequest, QueryBuilder};
use crate::api::url::encode_path_segment;
use crate::types::instrument::{
    CompactOptionChain, Cryptocurrency, EquityInstrument, EquityInstrumentInfo, EquityOption,
    FutureOption, FutureOptionProduct, FutureProduct, FuturesNestedOptionChain, NestedOptionChain,
    QuantityDecimalPrecision, Warrant,
};
use crate::types::instrument_filter::{ActiveEquityFilter, EquityFilter, FutureFilter};
use crate::{AsSymbol, TastyResult, TastyTrade};

impl TastyTrade {
    /// Details for one equity, including its trading flags.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_equity_info(
        &self,
        symbol: impl AsSymbol,
    ) -> TastyResult<EquityInstrumentInfo> {
        self.get(format!(
            "/instruments/equities/{}",
            encode_path_segment(&symbol.as_symbol().0)
        ))
        .await
    }

    /// One page of equities, filtered.
    ///
    /// [`EquityFilter::for_symbols`] is the common case; the default filter
    /// walks the whole listing a page at a time.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint answers without a pagination block, and when
    /// the listing arrives but nothing in it can be decoded — a defect in this
    /// crate's model rather than an empty result. A genuinely empty page is
    /// `Ok`.
    pub async fn list_equities(
        &self,
        filter: &EquityFilter,
    ) -> TastyResult<Paginated<EquityInstrument>> {
        let query = filter.to_query();
        self.get_with_query::<Items<EquityInstrument>, _, _>(
            "/instruments/equities",
            &query.pairs(),
        )
        .await
    }

    /// One page of currently active equities.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint answers without a pagination block, and as the
    /// other listings otherwise.
    pub async fn list_active_equities(
        &self,
        filter: &ActiveEquityFilter,
    ) -> TastyResult<Paginated<EquityInstrument>> {
        let query = filter.to_query();
        self.get_with_query::<Items<EquityInstrument>, _, _>(
            "/instruments/equities/active",
            &query.pairs(),
        )
        .await
    }

    /// One equity by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_equity(&self, symbol: impl AsSymbol) -> TastyResult<EquityInstrument> {
        self.get(format!(
            "/instruments/equities/{}",
            encode_path_segment(&symbol.as_symbol().0)
        ))
        .await
    }

    /// The flat option chain for an underlying.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_option_chains(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<Vec<EquityOption>> {
        let resp: Items<EquityOption> = self
            .get(format!(
                "/option-chains/{}",
                encode_path_segment(&underlying_symbol.as_symbol().0)
            ))
            .await?;
        resp.into_items()
    }

    /// The compact option chain for an underlying.
    ///
    /// Compact chains carry symbols without the per-contract detail, which is
    /// what you want when building a subscription list.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_compact_option_chain(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<CompactOptionChain> {
        // Through the generic verb like every other endpoint: it is the only
        // path that checks the status. Decoding by hand here used to put the
        // entire response body into the error message, so any caller logging a
        // parse failure logged the whole document.
        let resp: Items<CompactOptionChain> = self
            .get(format!(
                "/option-chains/{}/compact",
                encode_path_segment(&underlying_symbol.as_symbol().0)
            ))
            .await?;

        resp.into_items()?.into_iter().next().ok_or_else(|| {
            crate::TastyTradeError::Unknown(
                "No compact option chain data found in response".to_string(),
            )
        })
    }

    /// Option chains grouped by expiration and strike.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_nested_option_chains(
        &self,
        underlying_symbol: impl AsSymbol,
    ) -> TastyResult<Vec<NestedOptionChain>> {
        let resp: Items<NestedOptionChain> = self
            .get(format!(
                "/option-chains/{}/nested",
                encode_path_segment(&underlying_symbol.as_symbol().0)
            ))
            .await?;
        resp.into_items()
    }

    /// Equity options by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
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
        resp.into_items()
    }

    /// One equity option by symbol.
    ///
    /// `active` is the venue's documented filter for whether the option is
    /// currently available for trading with the broker. `None` omits it, which
    /// leaves the venue's own default in place.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_equity_option(
        &self,
        symbol: impl AsSymbol,
        active: Option<bool>,
    ) -> TastyResult<EquityOption> {
        // The hand-rolled envelope this replaced was `{ data: EquityOption }`,
        // which is what the generic verb decodes anyway — minus the status
        // check it never did and the body it put into the error message.
        let mut query = QueryBuilder::new();
        query.push_flag("active", active);
        self.get_with_query::<EquityOption, EquityOption, _>(
            format!(
                "/instruments/equity-options/{}",
                encode_path_segment(&symbol.as_symbol().0)
            ),
            &query.pairs(),
        )
        .await
    }

    /// One page of futures, filtered.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::list_equities`].
    pub async fn list_futures(
        &self,
        filter: &FutureFilter,
    ) -> TastyResult<Paginated<crate::types::instrument::Future>> {
        let query = filter.to_query();
        self.get_with_query::<Items<crate::types::instrument::Future>, _, _>(
            "/instruments/futures",
            &query.pairs(),
        )
        .await
    }

    /// One futures contract by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_future(
        &self,
        symbol: impl AsSymbol,
    ) -> TastyResult<crate::types::instrument::Future> {
        self.get(format!(
            "/instruments/futures/{}",
            encode_path_segment(&symbol.as_symbol().0)
        ))
        .await
    }

    /// One page of futures products.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::list_equities`].
    pub async fn list_future_products(
        &self,
        page: &PageRequest,
    ) -> TastyResult<Paginated<FutureProduct>> {
        let mut query = QueryBuilder::new();
        page.write_into(&mut query);
        self.get_with_query::<Items<FutureProduct>, _, _>(
            "/instruments/future-products",
            &query.pairs(),
        )
        .await
    }

    /// One futures product by exchange and code.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_future_product(
        &self,
        exchange: &str,
        code: &str,
    ) -> TastyResult<FutureProduct> {
        self.get(format!(
            "/instruments/future-products/{}/{}",
            encode_path_segment(exchange),
            encode_path_segment(code)
        ))
        .await
    }

    /// One page of futures-option products.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::list_equities`].
    pub async fn list_future_option_products(
        &self,
        page: &PageRequest,
    ) -> TastyResult<Paginated<FutureOptionProduct>> {
        let mut query = QueryBuilder::new();
        page.write_into(&mut query);
        self.get_with_query::<Items<FutureOptionProduct>, _, _>(
            "/instruments/future-option-products",
            &query.pairs(),
        )
        .await
    }

    /// One futures-option product, addressed by exchange and root symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_future_option_product_by_exchange(
        &self,
        exchange: &str,
        root_symbol: &str,
    ) -> TastyResult<FutureOptionProduct> {
        self.get(format!(
            "/instruments/future-option-products/{}/{}",
            encode_path_segment(exchange),
            encode_path_segment(root_symbol)
        ))
        .await
    }

    /// One futures-option product by root symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_future_option_product(
        &self,
        root_symbol: &str,
    ) -> TastyResult<FutureOptionProduct> {
        self.get(format!(
            "/instruments/future-option-products/{}",
            encode_path_segment(root_symbol)
        ))
        .await
    }

    /// The flat futures-option chain for a product.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_futures_option_chains(
        &self,
        product_code: &str,
    ) -> TastyResult<Vec<FutureOption>> {
        let resp: Items<FutureOption> = self
            .get(format!(
                "/futures-option-chains/{}",
                encode_path_segment(product_code)
            ))
            .await?;
        resp.into_items()
    }

    /// Futures-option chains grouped by expiration and strike.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_nested_futures_option_chains(
        &self,
        product_code: &str,
    ) -> TastyResult<Vec<FuturesNestedOptionChain>> {
        // This endpoint returns data in standard TastyApiResponse format with FuturesNestedOptionChain in data field
        let nested_chain: FuturesNestedOptionChain = self
            .get(format!(
                "/futures-option-chains/{}/nested",
                encode_path_segment(product_code)
            ))
            .await?;

        // Return as a vector with single item to match the expected return type
        Ok(vec![nested_chain])
    }

    /// Futures options by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
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
        resp.into_items()
    }

    /// One futures option by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_future_option(&self, symbol: impl AsSymbol) -> TastyResult<FutureOption> {
        let encoded_symbol = encode_path_segment(&symbol.as_symbol().0);
        self.get(format!("/instruments/future-options/{encoded_symbol}"))
            .await
    }

    /// Tradable cryptocurrencies.
    ///
    /// These trade in fractions, which is why quantities across this crate are
    /// `Decimal` rather than integers.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_cryptocurrencies(
        &self,
        symbols: &[impl AsSymbol],
    ) -> TastyResult<Vec<Cryptocurrency>> {
        let mut query = Vec::new();
        let mut symbol_strings = Vec::new();

        for symbol in symbols {
            symbol_strings.push(symbol.as_symbol().0.clone());
        }

        for symbol_str in &symbol_strings {
            query.push(("symbol[]", symbol_str.as_str()));
        }

        let resp: Items<Cryptocurrency> = self
            .get_with_query("/instruments/cryptocurrencies", &query)
            .await?;
        resp.into_items()
    }

    /// One cryptocurrency by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_cryptocurrency(&self, symbol: impl AsSymbol) -> TastyResult<Cryptocurrency> {
        let encoded_symbol = encode_path_segment(&symbol.as_symbol().0);
        self.get(format!("/instruments/cryptocurrencies/{encoded_symbol}"))
            .await
    }

    /// Tradable warrants.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
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
        resp.into_items()
    }

    /// One warrant by symbol.
    ///
    /// # Errors
    ///
    /// Fails when the venue does not recognise the symbol, and propagates its
    /// error otherwise.
    pub async fn get_warrant(&self, symbol: impl AsSymbol) -> TastyResult<Warrant> {
        self.get(format!(
            "/instruments/warrants/{}",
            encode_path_segment(&symbol.as_symbol().0)
        ))
        .await
    }

    /// How many decimal places each instrument type accepts for a quantity.
    ///
    /// Worth consulting before sizing an order: submitting more precision than
    /// the venue accepts is a rejection.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which is a
    /// defect in this crate's model rather than an empty result. A genuinely
    /// empty listing is `Ok`.
    pub async fn list_quantity_decimal_precisions(
        &self,
    ) -> TastyResult<Vec<QuantityDecimalPrecision>> {
        let resp: Items<QuantityDecimalPrecision> =
            self.get("/instruments/quantity-decimal-precisions").await?;
        resp.into_items()
    }
}
