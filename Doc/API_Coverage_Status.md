# tastytrade API — Coverage Status

Every REST endpoint published in the official OpenAPI specs at
<https://developer.tastytrade.com/open-api-spec/>, checked against what this
crate implements.

Source of truth: the swagger documents embedded in each `/open-api-spec/<area>/`
page (`__NEXT_DATA__` → `props.pageProps.specData`), newest version per area.
Snapshot taken 2026-08-03; the Orders spec was `order-api-swagger_20260427`.

## Summary

| Area | Endpoints | Implemented | Missing | % |
|------|-----------|-------------|---------|---|
| Account Status | 1 | 0 | 1 | 0% |
| Accounts and Customers | 4 | 2 | 2 | 50% |
| Backtesting | 7 | 0 | 7 | 0% |
| Balances and Positions | 4 | 3 | 1 | 75% |
| Instruments | 24 | 22 | 2 | 92% |
| Margin Requirements | 2 | 0 | 2 | 0% |
| Market Data | 1 | 0 | 1 | 0% |
| Market Metrics | 3 | 0 | 3 | 0% |
| Market Sessions | 11 | 0 | 11 | 0% |
| Net Liquidating Value History | 1 | 0 | 1 | 0% |
| Orders | 19 | 4 | 15 | 21% |
| Quote Alerts | 3 | 0 | 3 | 0% |
| Risk Parameters | 4 | 0 | 4 | 0% |
| Symbol Search | 1 | 0 | 1 | 0% |
| Transactions | 3 | 0 | 3 | 0% |
| Watchlists | 9 | 0 | 9 | 0% |
| **TOTAL** | **97** | **31** | **66** | **32%** |

Not counted above because they are documented in prose rather than in a swagger
document: the session lifecycle (`POST /sessions` — implemented) and OAuth2
(`POST /oauth/token` — not implemented).

## Account Status

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/trading-status` | — | ❌ |

Reports whether the account may trade, its margin type, PDT flag, options
level and any restrictions. One endpoint, and a caller has no other way to know
an order will be rejected for account reasons before sending it.

## Accounts and Customers

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /api-quote-tokens` | `quote_streamer_tokens()` | ✅ |
| `GET /customers/{customer_id}` | — | ❌ |
| `GET /customers/{customer_id}/accounts` | `accounts()` | ✅ |
| `GET /customers/{customer_id}/accounts/{account_number}` | — | ❌ |

`accounts()` hardcodes `me` as the customer id. `account(number)` fetches the
whole list and filters client-side instead of calling the single-account
endpoint.

## Backtesting

| Endpoint | Status |
|----------|--------|
| `GET /backtests` | ❌ |
| `POST /backtests` | ❌ |
| `GET /backtests/{id}` | ❌ |
| `GET /backtests/{id}/logs` | ❌ |
| `POST /backtests/{id}/cancel` | ❌ |
| `GET /available-dates` | ❌ |
| `POST /simulate-trade` | ❌ |

## Balances and Positions

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/balance-snapshots` | `balance_snapshot()` | ✅ |
| `GET /accounts/{account_number}/balances` | `balance()` | ✅ |
| `GET /accounts/{account_number}/balances/{currency}` | — | ❌ |
| `GET /accounts/{account_number}/positions` | `positions()` | ⚠️ |

`positions()` sends no query parameters. The endpoint accepts
`underlying-symbol[]`, `symbol`, `instrument-type`, `include-closed`,
`underlying-product-code`, `partition-keys[]`, `net-positions` and
`include-marks`; without them a caller cannot ask for closed positions or
filter server-side.

## Instruments

All implemented except the two search endpoints, which are new to the spec.

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /futures-option-chains/{symbol}` | `list_futures_option_chains()` | ✅ |
| `GET /futures-option-chains/{symbol}/nested` | `list_nested_futures_option_chains()` | ✅ |
| `POST /instruments/ai-search-token` | — | ❌ |
| `GET /instruments/cryptocurrencies` | `list_cryptocurrencies()` | ✅ |
| `GET /instruments/cryptocurrencies/{symbol}` | `get_cryptocurrency()` | ✅ |
| `GET /instruments/equities` | `list_equities()` | ✅ |
| `GET /instruments/equities/active` | `list_active_equities()` | ✅ |
| `GET /instruments/equities/{symbol}` | `get_equity()` | ✅ |
| `GET /instruments/equity-options/{symbol}` | `get_equity_option()` | ✅ |
| `GET /instruments/future-option-products` | `list_future_option_products()` | ✅ |
| `GET /instruments/future-option-products/{exchange}/{root_symbol}` | `get_future_option_product_by_exchange()` | ✅ |
| `GET /instruments/future-option-products/{root_symbol}` | `get_future_option_product()` | ✅ |
| `GET /instruments/future-options/{symbol}` | `get_future_option()` | ✅ |
| `GET /instruments/future-products` | `list_future_products()` | ✅ |
| `GET /instruments/future-products/{exchange}/{code}` | `get_future_product()` | ✅ |
| `GET /instruments/futures` | `list_futures()` | ✅ |
| `GET /instruments/futures/{symbol}` | `get_future()` | ✅ |
| `GET /instruments/quantity-decimal-precisions` | `list_quantity_decimal_precisions()` | ✅ |
| `GET /instruments/search` | — | ❌ |
| `GET /instruments/warrants` | `list_warrants()` | ✅ |
| `GET /instruments/warrants/{symbol}` | `get_warrant()` | ✅ |
| `GET /option-chains/{symbol}` | `list_option_chains()` / `option_chain_for()` | ✅ |
| `GET /option-chains/{symbol}/compact` | `get_compact_option_chain()` | ✅ |
| `GET /option-chains/{symbol}/nested` | `list_nested_option_chains()` / `nested_option_chain_for()` | ✅ |

The crate also implements `GET /instruments/equity-options` and
`GET /instruments/future-options` (the plural list forms) via
`list_equity_options()` and `list_future_options()`. Both work against the
venue but no longer appear in the published spec; keep them.

## Margin Requirements

| Endpoint | Status |
|----------|--------|
| `GET /margin/accounts/{account_number}/requirements` | ❌ |
| `POST /margin/accounts/{account_number}/dry-run` | ❌ |

## Market Data

| Endpoint | Status |
|----------|--------|
| `GET /market-data/by-type` | ❌ |

Snapshot quotes over REST for up to 100 symbols. Today the only way to get a
price out of this crate is to open a DXLink websocket.

## Market Metrics

| Endpoint | Status |
|----------|--------|
| `GET /market-metrics` | ❌ |
| `GET /market-metrics/historic-corporate-events/dividends/{symbol}` | ❌ |
| `GET /market-metrics/historic-corporate-events/earnings-reports/{symbol}` | ❌ |

IV rank, IV percentile, beta, liquidity rating, borrow rate, earnings dates.

## Market Sessions

| Endpoint | Status |
|----------|--------|
| `GET /market-time/sessions` | ❌ |
| `GET /market-time/sessions/current` | ❌ |
| `GET /market-time/equities/sessions/current` | ❌ |
| `GET /market-time/equities/sessions/next` | ❌ |
| `GET /market-time/equities/sessions/previous` | ❌ |
| `GET /market-time/equities/holidays` | ❌ |
| `GET /market-time/futures/sessions/current` | ❌ |
| `GET /market-time/futures/sessions/current/{instrument_collection}` | ❌ |
| `GET /market-time/futures/sessions/next/{instrument_collection}` | ❌ |
| `GET /market-time/futures/sessions/previous/{instrument_collection}` | ❌ |
| `GET /market-time/futures/holidays/{instrument_collection}` | ❌ |

## Net Liquidating Value History

| Endpoint | Status |
|----------|--------|
| `GET /accounts/{accountNumber}/net-liq/history` | ❌ |

## Orders

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/orders` | — | ❌ |
| `POST /accounts/{account_number}/orders` | `place_order()` / `place_reviewed_order()` | ✅ |
| `POST /accounts/{account_number}/orders/dry-run` | `dry_run()` / `review_order()` | ✅ |
| `GET /accounts/{account_number}/orders/live` | `live_orders()` | ✅ |
| `GET /accounts/{account_number}/orders/{id}` | — | ❌ |
| `PUT /accounts/{account_number}/orders/{id}` | — | ❌ |
| `PATCH /accounts/{account_number}/orders/{id}` | — | ❌ |
| `DELETE /accounts/{account_number}/orders/{id}` | `cancel_order()` | ✅ |
| `POST /accounts/{account_number}/orders/{id}/dry-run` | — | ❌ |
| `GET /accounts/{account_number}/complex-orders` | — | ❌ |
| `POST /accounts/{account_number}/complex-orders` | — | ❌ |
| `POST /accounts/{account_number}/complex-orders/dry-run` | — | ❌ |
| `GET /accounts/{account_number}/complex-orders/live` | — | ❌ |
| `GET /accounts/{account_number}/complex-orders/{id}` | — | ❌ |
| `PATCH /accounts/{account_number}/complex-orders/{id}` | — | ❌ |
| `DELETE /accounts/{account_number}/complex-orders/{id}` | — | ❌ |
| `POST /accounts/{account_number}/complex-orders/{id}/dry-run` | — | ❌ |
| `GET /customers/{customer_id}/orders` | — | ❌ |
| `GET /customers/{customer_id}/orders/live` | — | ❌ |

The order lifecycle is half-built. A caller can place and cancel, but cannot
read back a single order by id, search order history, or amend a working order
— cancel-replace is the normal way to reprice, and without it the only way to
move a limit is cancel-then-place, which loses queue position and can leave the
account flat between the two calls.

Complex orders (OCO, OTOCO, PAIRS) are absent entirely.

## Quote Alerts

| Endpoint | Status |
|----------|--------|
| `GET /quote-alerts` | ❌ |
| `POST /quote-alerts` | ❌ |
| `DELETE /quote-alerts/{alert_external_id}` | ❌ |

`AccountStreamer` already handles the `quote-alerts-subscribe` action, so the
streaming half exists with no way to create or list the alerts it delivers.

## Risk Parameters

| Endpoint | Status |
|----------|--------|
| `GET /accounts/{account_number}/margin-requirements/{underlying_symbol}/effective` | ❌ |
| `GET /accounts/{account_number}/position-limit` | ❌ |
| `GET /margin-requirements-public-configuration` | ❌ |
| `GET /span/rows` | ❌ |

## Symbol Search

| Endpoint | Status |
|----------|--------|
| `GET /symbols/search/{symbol}` | ❌ |

## Transactions

| Endpoint | Status |
|----------|--------|
| `GET /accounts/{account_number}/transactions` | ❌ |
| `GET /accounts/{account_number}/transactions/total-fees` | ❌ |
| `GET /accounts/{account_number}/transactions/{id}` | ❌ |

Fills, fees, commissions, dividends, assignments. Nothing in the crate can
reconstruct what an account actually did.

## Watchlists

| Endpoint | Status |
|----------|--------|
| `GET /public-watchlists` | ❌ |
| `GET /public-watchlists/{watchlist_name}` | ❌ |
| `GET /watchlists` | ❌ |
| `POST /watchlists` | ❌ |
| `GET /watchlists/{watchlist_name}` | ❌ |
| `PUT /watchlists/{watchlist_name}` | ❌ |
| `DELETE /watchlists/{watchlist_name}` | ❌ |
| `GET /pairs-watchlists` | ❌ |
| `GET /pairs-watchlists/{pairs_watchlist_name}` | ❌ |

## Gaps outside the endpoint list

- **Session lifecycle.** `POST /sessions` is implemented. There is no logout,
  and the `remember-token` the venue returns in `LoginResponse` is parsed and
  then dropped — nothing can re-authenticate with it, so `TASTYTRADE_REMEMBER_ME`
  buys the caller nothing today.
- **OAuth2.** `POST /oauth/token` and the refresh-token flow are not
  implemented. This is the auth path tastytrade documents for third-party
  applications; session tokens are the personal-use path.
- **Streaming market data.** Candle events are documented under
  `/streaming-market-data/#candle-events` and the crate does not subscribe to
  them, so there is no historical bar data by any route.
- **Account streamer.** The five documented actions (`heartbeat`, `connect`,
  `public-watchlists-subscribe`, `quote-alerts-subscribe`,
  `user-message-subscribe`) are all present in `SubRequestAction`.
