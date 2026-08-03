# tastytrade API — Coverage Status

Every REST endpoint published in the official OpenAPI specs at
<https://developer.tastytrade.com/open-api-spec/>, checked against what this
crate implements.

Source of truth: the swagger documents embedded in each `/open-api-spec/<area>/`
page (`__NEXT_DATA__` → `props.pageProps.specData`), newest version per area.
Snapshot taken 2026-08-03; the Orders spec was `order-api-swagger_20260427` and
the Instruments spec `instruments-api-swagger_20250715`.

Three states, not two. ✅ implemented · ❌ published and not yet implemented ·
❔ **named somewhere official but not in the current public API document**, so
there is no contract to implement against. Only the first two are counted in
the totals; an endpoint in the third state is not a backlog item, and the
difference is exactly what
[#90](https://github.com/joaquinbejar/tastytrade/issues/90) is about.

## Summary

| Area | Endpoints | Implemented | Missing | % | Issue |
|------|-----------|-------------|---------|---|-------|
| Account Status | 1 | 0 | 1 | 0% | [#73](https://github.com/joaquinbejar/tastytrade/issues/73) |
| Accounts and Customers | 4 | 4 | 0 | 100% | [#75](https://github.com/joaquinbejar/tastytrade/issues/75) |
| Backtesting | 7 | 0 | 7 | 0% | [#84](https://github.com/joaquinbejar/tastytrade/issues/84) |
| Balances and Positions | 4 | 4 | 0 | 100% | [#74](https://github.com/joaquinbejar/tastytrade/issues/74) |
| Instruments | 24 | 24 | 0 | 100% | [#82](https://github.com/joaquinbejar/tastytrade/issues/82) |
| Margin Requirements | 2 | 0 | 2 | 0% | [#78](https://github.com/joaquinbejar/tastytrade/issues/78) |
| Market Data | 1 | 0 | 1 | 0% | [#76](https://github.com/joaquinbejar/tastytrade/issues/76) |
| Market Metrics | 3 | 0 | 3 | 0% | [#77](https://github.com/joaquinbejar/tastytrade/issues/77) |
| Market Sessions | 11 | 0 | 11 | 0% | [#79](https://github.com/joaquinbejar/tastytrade/issues/79) |
| Net Liquidating Value History | 1 | 0 | 1 | 0% | [#83](https://github.com/joaquinbejar/tastytrade/issues/83) |
| Orders | 19 | 4 | 15 | 21% | [#70](https://github.com/joaquinbejar/tastytrade/issues/70), [#71](https://github.com/joaquinbejar/tastytrade/issues/71) |
| Quote Alerts | 3 | 0 | 3 | 0% | [#81](https://github.com/joaquinbejar/tastytrade/issues/81) |
| Risk Parameters | 4 | 0 | 4 | 0% | [#78](https://github.com/joaquinbejar/tastytrade/issues/78) |
| Symbol Search | 1 | 1 | 0 | 100% | [#82](https://github.com/joaquinbejar/tastytrade/issues/82) |
| Transactions | 3 | 3 | 0 | 100% | [#72](https://github.com/joaquinbejar/tastytrade/issues/72) |
| Watchlists | 9 | 0 | 9 | 0% | [#80](https://github.com/joaquinbejar/tastytrade/issues/80) |
| **TOTAL** | **97** | **40** | **57** | **41%** | |

Not counted above because it is documented in prose rather than in a swagger
document: OAuth2 (`POST /oauth/token` — implemented, both grants), tracked in
[#85](https://github.com/joaquinbejar/tastytrade/issues/85). The session
lifecycle it replaced (`POST /sessions`) was decommissioned by the venue on
2026-02-11.

Streaming is not counted either; see the section at the end.
`Doc/ROADMAP.md` sequences all of this by priority.

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
| `GET /customers/{customer_id}` | `customer()` / `customer_by_id()` / `find_customer()` | ✅ |
| `GET /customers/{customer_id}/accounts` | `accounts()` | ✅ |
| `GET /customers/{customer_id}/accounts/{account_number}` | `account_by_number()` | ✅ |

`account(number)` no longer fetches the whole listing and filters it here. That
mattered because `Items<T>` skips an item it cannot parse, so a **sibling**
account failing to deserialize made the requested one disappear and the call
returned `Ok(None)` — indistinguishable from "this session cannot see that
account", and exactly the shape of the `is-test-drive` bug in
[#5](https://github.com/joaquinbejar/tastytrade/issues/5). `Ok(None)` now means
the venue returned `404`.

`Customer` is the most sensitive object in the API — names, addresses, tax
identifiers, birth dates, net worth. Nothing in it renders itself: `Debug` and
`Display` print a field count. `find_customer()` sends the documented
`allow-missing`.

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
| `GET /accounts/{account_number}/balance-snapshots` | `balance_snapshots()` | ✅ |
| `GET /accounts/{account_number}/balances` | `balances()` / `balance()` | ✅ |
| `GET /accounts/{account_number}/balances/{currency}` | `balance_in()` | ✅ |
| `GET /accounts/{account_number}/positions` | `positions()` / `positions_matching()` | ✅ |

`/balances` answers with an **`items` envelope**, not a single object — the
venue changed it on 2024-05-01 and this crate decoded `data` straight into a
`Balance`, so the call could only fail. `balances()` returns every currency
row; `balance()` is the single-row shortcut and refuses when the account holds
more than one rather than picking a currency for the caller.

`positions_matching(&PositionFilter)` exposes all eight documented filters, with
`underlying-symbol[]` and `partition-keys[]` as repeated keys. `positions()` is
unchanged and sends no query at all.

`BalanceSnapshotFilter` covers the whole snapshot query. `time-of-day` is a
constructor argument because the venue marks it required, and a single day and
a date range are one enum so the contradictory query is unrepresentable. Its
value used to go out as `Eod` rather than `EOD`: `SnapshotTimeOfDay`'s
`Display` was the derived `Debug`, and the query was built from it.

## Instruments

Complete against `instruments-api-swagger_20250715`.

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /futures-option-chains/{symbol}` | `list_futures_option_chains()` | ✅ |
| `GET /futures-option-chains/{symbol}/nested` | `list_nested_futures_option_chains()` | ✅ |
| `POST /instruments/ai-search-token` | `ai_search_token()` | ✅ |
| `GET /instruments/cryptocurrencies` | `list_cryptocurrencies()` | ✅ |
| `GET /instruments/cryptocurrencies/{symbol}` | `get_cryptocurrency()` | ✅ |
| `GET /instruments/equities` | `list_equities(&EquityFilter)` → `Paginated<T>` | ✅ |
| `GET /instruments/equities/active` | `list_active_equities(&ActiveEquityFilter)` → `Paginated<T>` | ✅ |
| `GET /instruments/equities/{symbol}` | `get_equity()` | ✅ |
| `GET /instruments/equity-options/{symbol}` | `get_equity_option()` | ✅ |
| `GET /instruments/future-option-products` | `list_future_option_products(&PageRequest)` → `Paginated<T>` | ✅ |
| `GET /instruments/future-option-products/{exchange}/{root_symbol}` | `get_future_option_product_by_exchange()` | ✅ |
| `GET /instruments/future-option-products/{root_symbol}` | `get_future_option_product()` | ✅ |
| `GET /instruments/future-options/{symbol}` | `get_future_option()` | ✅ |
| `GET /instruments/future-products` | `list_future_products(&PageRequest)` → `Paginated<T>` | ✅ |
| `GET /instruments/future-products/{exchange}/{code}` | `get_future_product()` | ✅ |
| `GET /instruments/futures` | `list_futures(&FutureFilter)` → `Paginated<T>` | ✅ |
| `GET /instruments/futures/{symbol}` | `get_future()` | ✅ |
| `GET /instruments/quantity-decimal-precisions` | `list_quantity_decimal_precisions()` | ✅ |
| `GET /instruments/search` | `search_instruments()` | ✅ |
| `GET /instruments/warrants` | `list_warrants()` | ✅ |
| `GET /instruments/warrants/{symbol}` | `get_warrant()` | ✅ |
| `GET /option-chains/{symbol}` | `list_option_chains()` / `option_chain_for()` | ✅ |
| `GET /option-chains/{symbol}/compact` | `get_compact_option_chain()` | ✅ |
| `GET /option-chains/{symbol}/nested` | `list_nested_option_chains()` / `nested_option_chain_for()` | ✅ |

The crate also implements `GET /instruments/equity-options` and
`GET /instruments/future-options` (the plural list forms) via
`list_equity_options()` and `list_future_options()`. Both work against the
venue but no longer appear in the published spec; keep them.

**They keep returning `Vec<T>`**, unlike every other listing. The `20250715`
release note says they paginate, and the spec published the same day does not
describe them at all — so there is nothing to check the return type against,
and switching them to `Paginated<T>` on the release note alone would make every
existing call fail if the note is wrong. The probe in
[#90](https://github.com/joaquinbejar/tastytrade/issues/90) covers
`/instruments/equity-options` as one of its controls; running it settles this
at the same time.

### Pagination and filters

Every listing above that the spec paginates returns `Paginated<T>` and takes a
typed filter. The filters are `EquityFilter`, `ActiveEquityFilter` and
`FutureFilter`; `PageRequest` is the page itself and is shared. An unset
parameter is **omitted**, so the venue's own defaults survive — that matters
for `only-active-futures`, which the venue defaults to true.

### Named in a release note, absent from the spec — [#90](https://github.com/joaquinbejar/tastytrade/issues/90)

Two routes are described by the official release notes and documented nowhere
else. They are **not** counted in the totals above, in either column: counting
them as missing would claim they exist, and counting them as retired would
claim they do not.

| Endpoint | State | Determined |
|----------|-------|------------|
| `GET /instruments/equity-deliverables` | ❔ not in the current public API document | 2026-08-03 |
| `GET /instruments/future-spreads` | ❔ not in the current public API document | 2026-08-03 |

**Legend.** ✅ implemented · ❌ published and not yet implemented · ❔ not in
the current public API document, so there is no contract to implement against.
The third state is the point of this section: it is not a backlog item.

#### The evidence

The release note stamped `20250715` at <https://developer.tastytrade.com/release-notes/>
says response data is now paginated for eight endpoints, listing
`GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
among them. So both existed on 2025-07-15.

The Instruments OpenAPI document currently served from
<https://developer.tastytrade.com/open-api-spec/instruments/> is
`instruments-api-swagger_20250715.json` — the **same date** — and contains 24
paths, neither of them among them.

#### Why that is not enough to declare them retired

The same release note names two more endpoints that the same-day spec also
omits: `GET /instruments/equity-options` and `GET /instruments/future-options`,
the plural list forms. Both are present in the earlier spec capture kept at
`Doc/Instruments.json`, both are implemented here as `list_equity_options()`
and `list_future_options()`, and both answer.

Four of the eight endpoints in that release note are missing from the spec
published beside it, and at least two of those four demonstrably still work.
Absence from the document is therefore evidence about the **document**, not
about the API. Deriving a client contract for `equity-deliverables` or
`future-spreads` from the release note alone would be inventing one — an
`items` envelope and a pagination block is the entire published description,
with no field list, no filters and no response schema.

#### What would settle it

One read-only GET per route against a live host. That is
`examples/instruments/src/bin/probe_undocumented.rs`, which probes both routes
plus the two controls above and reports the status and envelope shape:

```shell
TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin probe_undocumented
```

It has **not been run**: this checkout has no OAuth application or grant, the
same blocker as [#96](https://github.com/joaquinbejar/tastytrade/issues/96). A
`404` retires the routes; anything else means they exist and the reply itself
is the contract to model. Record the outcome here with its date either way.

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

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /symbols/search/{symbol}` | `search_symbols()` | ✅ |

Its own area with its own document (`symbol-search-server-swagger.json`), which
is why it is not one of the 24 Instruments paths. The query is a **path
segment**, so it goes through the shared encoder from
[#89](https://github.com/joaquinbejar/tastytrade/issues/89) — `BRK/B` and `/ES`
both carry separators.

## Transactions

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/transactions` | `transactions()` | ✅ |
| `GET /accounts/{account_number}/transactions/total-fees` | `total_fees()` | ✅ |
| `GET /accounts/{account_number}/transactions/{id}` | `transaction()` | ✅ |

`TransactionType`, `TransactionSubType` and `TransactionAction` are
`wire_enum!` sets built from the value lists in the venue's own API guide, each
with an `Unknown(String)` arm — a strict enum would make a new transaction kind
disappear from a ledger through `Items<T>` without an error.

`TransactionAction` is deliberately **not** `types::order::Action`. That one is
the order-placement enum and is strict on purpose: an order with an action this
crate does not recognise is an order nobody should be able to build. This is the
read side, where tolerance keeps a ledger complete.

`type` and `types` are mutually exclusive at the venue, so they are one enum
(`TransactionTypes`) and a request carrying both cannot be built.

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

## Authentication — OAuth2

Implemented in [#85](https://github.com/joaquinbejar/tastytrade/issues/85).

**`POST /sessions` was fully decommissioned on 2026-02-11.** From the official
release notes: *"Legacy /sessions authentication has been fully decommissioned.
If you are still using POST /sessions for your API application you likely are
experiencing login issues. Please switch over to OAuth2 immediately."*

| Endpoint / capability | Implemented |
|---|---|
| `POST /oauth/token`, `grant_type=refresh_token` | ✅ `TastyTrade::connect` |
| `POST /oauth/token`, `grant_type=authorization_code` | ✅ `TastyTrade::connect_with_authorization_code` |
| Authorization URL, both environments | ✅ `AuthorizationRequest::authorize_url` |
| `state` round-trip | ✅ `AuthorizationRequest::verify_state` |
| Access-token expiry tracking and renewal | ✅ `OAuthSession`, 60s margin |
| `Authorization: Bearer` on every REST request | ✅ |
| `Bearer `-prefixed `auth-token` on the account websocket | ✅ |
| `POST /sessions`, `DELETE /sessions`, remember-token | ❌ retired by the venue, removed here |

Access tokens last about 15 minutes. Renewal happens **before** a request, not
as a retry after a `401`: a `POST` that may have placed an order is never
replayed. A session is bound to the deployment it authenticated against, so
neither a token nor the client secret can follow a changed `base_url`
somewhere else.

The logout and remember-token work originally filed here is moot: those are
surfaces of a retired API. `TASTYTRADE_REMEMBER_ME` is no longer read.

## Streaming

Two websockets, neither counted in the 97 REST endpoints. On both, the crate
subscribes to more than it can deliver.

### DXLink market data — [#86](https://github.com/joaquinbejar/tastytrade/issues/86)

dxlink 0.3.1 models eleven `MarketEvent` variants. The crate routes all eleven.

| Event type | Routed | Notes |
|---|---|---|
| `Quote` | ✅ | |
| `Trade` | ✅ | regular session only |
| `TradeETH` | ✅ | the only route to an extended-hours last price |
| `Greeks` | ✅ | |
| `Candle` | ✅ | the only route to a price series anywhere in this crate |
| `Summary` | ✅ | |
| `TimeAndSale` | ✅ | |
| `Profile` | ✅ | |
| `Underlying` | ✅ | |
| `TheoPrice` | ✅ | |
| `Series` | ✅ | |

`event_kind()` still matches every variant with **no wildcard**, and
`MarketEvent` is still not `#[non_exhaustive]`, so a twelfth upstream variant
breaks the build rather than being silently dropped. That tripwire is what
produced #86: `0.3.1` added `TradeETH` and `Series` and the build stopped
compiling.

The channel is configured for the event types its subscriptions asked for,
reconfigured when a later subscription wants more. It used to be set up at
connect time for a hardcoded Quote, Trade and Greeks, so any other
subscription was accepted locally and then delivered nothing.

Candles are addressed by a symbol carrying their period — `AAPL{=5m}` — and
routing is keyed by `(streamer symbol, event type)`, so two periods of one
underlying, and two subscriptions asking for different types on one symbol,
never receive each other's events. `from_time` is required on
`add_candles`; a reconnect resumes one millisecond past the last bar
delivered.

**Not verified against the venue.** Event delivery was only fixed in #66/#69
and has never been confirmed live; `/smoke` against certification is what would
settle it.

### Account websocket — [#87](https://github.com/joaquinbejar/tastytrade/issues/87)

The four documented actions are implemented, plus `user-message-subscribe`,
which the crate keeps although tastytrade no longer documents it.

| Notification | Typed | Source of the schema |
|---|---|---|
| `Order` | ✅ legs and fills included | `order-api-swagger_20260427` |
| `AccountBalance` | ✅ | `account-positions-api-swagger_20240501` |
| `CurrentPosition` | ✅ | `account-positions-api-swagger_20240501` |
| `QuoteAlert` | ✅ | `quote-alerts-api-swagger` |
| `PublicWatchlists` | ✅ | `watchlists-api-swagger` |
| `OrderChain` | ⚠️ delivered untyped | no captured frame |
| `ExternalTransaction` | ⚠️ delivered untyped | no captured frame |
| `UserMessage` | ⚠️ delivered untyped | undocumented |
| `ComplexOrder` | ⚠️ delivered untyped | [#71](https://github.com/joaquinbejar/tastytrade/issues/71) |
| `TradingStatus` | ⚠️ delivered untyped | [#73](https://github.com/joaquinbejar/tastytrade/issues/73) |
| `UnderlyingYearGainSummary` | ⚠️ delivered untyped | no captured frame |
| anything else | ⚠️ delivered untyped | — |

"Delivered untyped" means `NotificationPayload::Unsupported`, which carries the
payload. Nothing is discarded any more: an unrecognised `type`, a payload that
does not match its model, and a frame that is neither a notification nor an
acknowledgement all reach the caller. Only bytes that are not JSON do not.

The frames themselves are in `Doc/account_streaming_frames.md`, marked
documented-or-derived. **None of them is a captured venue frame**; capturing
one per notification needs `/smoke` against certification.

### DXLink as an account transport — [#54](https://github.com/joaquinbejar/tastytrade/issues/54), closed as not planned

The premise did not hold. Two independent reasons:

1. **tastytrade does not publish account data over DXLink.** Account
   notifications go over the tastytrade account websocket at
   `wss://streamer.[cert.]tastyworks.com`, authenticated with the session token
   as the `auth-token` field of each `SubRequest`. DXLink is the market-data
   streamer, reached through `GET /api-quote-tokens`. Different services,
   different credentials.
2. **In dxFeed, `Order` is order-book depth**, not "your order filled". A
   decoder for it would produce correct events of the wrong kind. `Order` and
   `Message` also have no decoder in dxlink 0.3 (`compact_fields()` → `None`),
   so a row of either aborts the batch decode rather than being skipped.

#17 removing DXLink from `AccountStreamer` was right for a stronger reason than
recorded at the time: not merely unproven, but the wrong service. What remains
of the account-streaming gap is tracked in
[#87](https://github.com/joaquinbejar/tastytrade/issues/87).

### Upstream (dxlink 0.3)

Nothing this crate needs for market data is missing upstream — `MarketEvent`
decodes all nine types the tastytrade docs describe, `CandleEvent` is complete,
and `FeedSubscription::from_time` already exists. Six `EventType` variants are
declared but undecoded (`Order`, `TradeETH`, `SpreadOrder`, `Series`,
`Configuration`, `Message`). Two are worth having and are filed:
[DXlink#66](https://github.com/joaquinbejar/DXlink/issues/66) (`TradeETH`,
extended-hours prints) and
[DXlink#67](https://github.com/joaquinbejar/DXlink/issues/67) (`Series`,
per-expiration option data). The other four are order-book depth — likely
outside this entitlement — or protocol plumbing.
