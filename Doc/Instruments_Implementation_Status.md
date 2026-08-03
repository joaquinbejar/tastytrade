# Instruments API — Implementation Status

Every path in the Instruments OpenAPI document, checked against what this crate
implements.

Source: the swagger embedded in
<https://developer.tastytrade.com/open-api-spec/instruments/>
(`__NEXT_DATA__` → `props.pageProps.specData`), version
**`instruments-api-swagger_20250715`**, read 2026-08-03. That document lists
**24 paths**.

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
| Search | 2 | 2 |
| Other | 1 | 1 |
| Option chains | 3 | 3 |
| **TOTAL** | **24** | **24** |

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

Cryptocurrency **trading** through the API is disabled by the venue as of
2026-06-29. Discovery and market data are unaffected; order routing is tracked
in [#91](https://github.com/joaquinbejar/tastytrade/issues/91).

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
| `POST /instruments/ai-search-token` | `ai_search_token()` | Mints a Telescope credential; redacted in `Debug`/`Display` |

`GET /symbols/search/{symbol}` is `search_symbols()`. It belongs to the separate
Symbol Search area (`symbol-search-server-swagger.json`) and is not one of the
24 paths counted here.

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

Two plural list forms this crate calls and the current spec does not describe:

| Endpoint | Method | Return |
|----------|--------|--------|
| `GET /instruments/equity-options` | `list_equity_options()` | `Vec<T>` |
| `GET /instruments/future-options` | `list_future_options()` | `Vec<T>` |

Both appear in the earlier spec capture at `Doc/Instruments.json`, and the
`20250715` release note names both as newly paginated. They keep returning
`Vec<T>` rather than `Paginated<T>` precisely because there is no current
document to check a new return type against, and switching them on a release
note alone would break every existing call if the note is wrong. The reasoning
and the probe that settles it are in `Doc/API_Coverage_Status.md` under
[#90](https://github.com/joaquinbejar/tastytrade/issues/90).

## Not in the document at all

`GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
are named in the same release note and appear in no OpenAPI document. Not
implemented, and not counted above as missing —
see `Doc/API_Coverage_Status.md`.

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
