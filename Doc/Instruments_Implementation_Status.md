# Instruments API — Implementation Status

Every path in the Instruments OpenAPI document, checked against what this crate
implements.

Source: the swagger embedded in
<https://developer.tastytrade.com/open-api-spec/instruments/>
(`__NEXT_DATA__` → `props.pageProps.specData`), version
**`20250715`**, re-read 2026-08-20. That default document lists **23 paths**.

The area-wide matrix is `Doc/API_Coverage_Status.md`; this file is the
endpoint-by-endpoint detail.

## Summary

| Group | Endpoints | Implemented |
|-------|-----------|-------------|
| Futures option chains | 2 | 2 |
| Cryptocurrencies | 2 | 2 |
| Equities | 3 | 3 |
| Equity options | 1 | 1 |
| Future options | 1 | 1 |
| Future products and future option products | 5 | 5 |
| Futures | 2 | 2 |
| Warrants | 2 | 2 |
| Search | 1 | 1 |
| Other | 1 | 1 |
| Option chains | 3 | 3 |
| **TOTAL** | **23** | **23** |

## Endpoints

### Futures option chains

| Endpoint | Method |
|----------|--------|
| `GET /futures-option-chains/{symbol}` | `list_futures_option_chains()` |
| `GET /futures-option-chains/{symbol}/nested` | `list_nested_futures_option_chains()` |

### Cryptocurrencies

| Endpoint | Method |
|----------|--------|
| `GET /instruments/cryptocurrencies` | `list_cryptocurrencies()` |
| `GET /instruments/cryptocurrencies/{symbol}` | `get_cryptocurrency()` |

Cryptocurrency **trading** through the API was disabled by the venue on
2026-06-29. Discovery and market data are unaffected; order routing for a
cryptocurrency leg is refused locally, and the decision lives in one constant
(`CRYPTOCURRENCY_TRADING_ENABLED`) so restoring it is a one-line change.

### Equities

| Endpoint | Method | Notes |
|----------|--------|-------|
| `GET /instruments/equities` | `list_equities(&EquityFilter)` | `Paginated<T>`; `symbol[]`, `is-etf`, `is-index`, `lendability` |
| `GET /instruments/equities/active` | `list_active_equities(&ActiveEquityFilter)` | `Paginated<T>`; `lendability` |
| `GET /instruments/equities/{symbol}` | `get_equity()` / `get_equity_info()` | Two return shapes over one route |

### Equity options

| Endpoint | Method | Notes |
|----------|--------|-------|
| `GET /instruments/equity-options/{symbol}` | `get_equity_option(symbol, active)` | `active` is the venue's documented filter |

### Future options

| Endpoint | Method |
|----------|--------|
| `GET /instruments/future-options/{symbol}` | `get_future_option()` |

### Future products and future option products

| Endpoint | Method | Notes |
|----------|--------|-------|
| `GET /instruments/future-products` | `list_future_products(&PageRequest)` | `Paginated<T>` |
| `GET /instruments/future-products/{exchange}/{code}` | `get_future_product()` | |
| `GET /instruments/future-option-products` | `list_future_option_products(&PageRequest)` | `Paginated<T>` |
| `GET /instruments/future-option-products/{exchange}/{root_symbol}` | `get_future_option_product_by_exchange()` | |
| `GET /instruments/future-option-products/{root_symbol}` | `get_future_option_product()` | |

### Futures

| Endpoint | Method | Notes |
|----------|--------|-------|
| `GET /instruments/futures` | `list_futures(&FutureFilter)` | `Paginated<T>`; `symbol[]`, `product-code[]`, `security-id[]`, `exchange`, `only-active-futures` |
| `GET /instruments/futures/{symbol}` | `get_future()` | |

### Warrants

| Endpoint | Method |
|----------|--------|
| `GET /instruments/warrants` | `list_warrants()` |
| `GET /instruments/warrants/{symbol}` | `get_warrant()` |

### Search

| Endpoint | Method | Notes |
|----------|--------|-------|
| `GET /instruments/search` | `search_instruments(&InstrumentSearchFilter)` | Filters are **comma-joined**, not repeated keys; `limit` capped at 100 locally |

`GET /symbols/search/{symbol}` is `search_symbols()`. It belongs to the separate
Symbol Search area (`symbol-search-server-swagger.json`) and is not one of the
23 paths counted here.

### Other

| Endpoint | Method |
|----------|--------|
| `GET /instruments/quantity-decimal-precisions` | `list_quantity_decimal_precisions()` |

### Option chains

| Endpoint | Method |
|----------|--------|
| `GET /option-chains/{symbol}` | `list_option_chains()` / `option_chain_for()` |
| `GET /option-chains/{symbol}/compact` | `get_compact_option_chain()` |
| `GET /option-chains/{symbol}/nested` | `list_nested_option_chains()` / `nested_option_chain_for()` |

## Implemented but no longer in the document

Three methods this crate calls and the current spec does not describe:

| Endpoint | Method | Return |
|----------|--------|--------|
| `GET /instruments/equity-options` | `list_equity_options()` | `Vec<T>` |
| `GET /instruments/future-options` | `list_future_options()` | `Vec<T>` |
| `POST /instruments/ai-search-token` | `ai_search_token()` | redacted `AiSearchToken` |

The two plural collections appear in the earlier spec capture at
`Doc/Instruments.json`, and the `20250715` release note names both as newly
paginated. They keep returning
`Vec<T>` rather than `Paginated<T>` precisely because there is no current
document to check a new return type against, and switching them on a release
note alone would break every existing call if the note is wrong. The reasoning
and the certification plus production probes that leave the contract
access-restricted and unverified are in `Doc/API_Coverage_Status.md` under
[#90](https://github.com/joaquinbejar/tastytrade/issues/90).

`ai_search_token()` remains for compatibility but disappeared from the current
default document as well. Its opaque credential stays redacted in `Debug`,
`Display` and errors.

The distinction matters: the current `20250715` Swagger documents
`GET /instruments/equity-options/{symbol}` and that singular lookup is the
supported `get_equity_option()` contract. It does not document the plural
collection called by `list_equity_options()`.

## Not in the document at all

`GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
are named in the same release note and appear in no OpenAPI document. Not
implemented, and not counted above as missing. Both certification on 2026-08-04
and an authorised production probe on 2026-08-20 returned `403` beside a `200`
documented control and a `404` negative control. No successful payload exists
to define fields, filters or pagination. Because access policy can run before
final route matching, those `403`s do not prove route existence either; see
`Doc/API_Coverage_Status.md`.

## Type notes

| Type | Worth knowing |
|------|---------------|
| `EquityInstrument` | `lendability` stays `Option<String>`; the filter side uses the `Lendability` wire enum |
| `Future`, `FutureProduct` | Tick sizes and closing-only dates are `Option<T>` — cert omits fields production sends |
| `CompactOptionChain` | `settlement_type`, `expiration_type`, symbols and streamer symbols are all optional |
| `InstrumentSearchResult` | Everything except `symbol` is `Option<T>`; the swagger marks no field required |
| `AiSearchToken` | Holds the whole response object because the spec publishes **no** response schema; redacted in `Debug`, `Display` and errors |

Every monetary or tick-size field is `Decimal`. Symbols, exchange and clearing
codes, CUSIPs and descriptions stay `String` on purpose.
