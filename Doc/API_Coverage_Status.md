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
| Account Status | 1 | 1 | 0 | 100% | [#73](https://github.com/joaquinbejar/tastytrade/issues/73) |
| Accounts and Customers | 4 | 4 | 0 | 100% | [#75](https://github.com/joaquinbejar/tastytrade/issues/75) |
| Backtesting | 7 | 7 | 0 | 100% | [#84](https://github.com/joaquinbejar/tastytrade/issues/84) |
| Balances and Positions | 4 | 4 | 0 | 100% | [#74](https://github.com/joaquinbejar/tastytrade/issues/74) |
| Instruments | 24 | 24 | 0 | 100% | [#82](https://github.com/joaquinbejar/tastytrade/issues/82) |
| Margin Requirements | 2 | 2 | 0 | 100% | [#78](https://github.com/joaquinbejar/tastytrade/issues/78) |
| Market Data | 1 | 1 | 0 | 100% | [#76](https://github.com/joaquinbejar/tastytrade/issues/76) |
| Market Metrics | 3 | 3 | 0 | 100% | [#77](https://github.com/joaquinbejar/tastytrade/issues/77) |
| Market Sessions | 11 | 11 | 0 | 100% | [#79](https://github.com/joaquinbejar/tastytrade/issues/79) |
| Net Liquidating Value History | 1 | 1 | 0 | 100% | [#83](https://github.com/joaquinbejar/tastytrade/issues/83) |
| Orders | 19 | 19 | 0 | 100% | [#70](https://github.com/joaquinbejar/tastytrade/issues/70), [#71](https://github.com/joaquinbejar/tastytrade/issues/71) |
| Quote Alerts | 3 | 3 | 0 | 100% | [#81](https://github.com/joaquinbejar/tastytrade/issues/81) |
| Risk Parameters | 4 | 4 | 0 | 100% | [#78](https://github.com/joaquinbejar/tastytrade/issues/78) |
| Symbol Search | 1 | 1 | 0 | 100% | [#82](https://github.com/joaquinbejar/tastytrade/issues/82) |
| Transactions | 3 | 3 | 0 | 100% | [#72](https://github.com/joaquinbejar/tastytrade/issues/72) |
| Watchlists | 9 | 9 | 0 | 100% | [#80](https://github.com/joaquinbejar/tastytrade/issues/80) |
| **TOTAL** | **97** | **97** | **0** | **100%** | |

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
| `GET /accounts/{account_number}/trading-status` | `trading_status()` | ✅ |

Reports whether the account may trade, its margin type, PDT flag, options
level, the live day-trade count and every feature flag. One request, and it
answers before the venue answers with a rejection.

All 44 fields are `Option<T>` per the `AccountDetails` precedent: a flag the
broker did not send is unknown, never `false`. `is_blocked()` is a convenience,
and `is_known_blocked()` sits beside it so a caller can tell "the account is
fine" from "the venue did not say" — collapsing that would undo the point of
the `Option`.

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

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /backtests` | `backtests()` | ✅ |
| `POST /backtests` | `create_backtest()` | ✅ |
| `GET /backtests/{id}` | `backtest()` | ✅ |
| `GET /backtests/{id}/logs` | `backtest_logs()` | ✅ |
| `POST /backtests/{id}/cancel` | `cancel_backtest()` | ✅ |
| `GET /available-dates` | `available_dates()` | ✅ |
| `POST /simulate-trade` | `simulate_trade()` | ✅ |

**The host question is answered.** The published document does declare a
server — `https://backtester.vast.tastyworks.com` — as an OpenAPI 3 `servers`
entry, which is why it did not show up as the `host` field the Swagger 2 areas
use. It is a **separate host**, and there is exactly **one**: no cert/production
pair, so this crate does not invent a second URL.

That leaves `environment()` correct as it stands. It derives from the configured
`base_url`, which is still a tastytrade API host — the session authenticated
against cert or production and a backtest run by that session is still that
session's. Nothing about the derivation needed changing; a backtest error names
the session's environment, which is what a caller needs to know.

The verbs were generalised rather than copied (`get_with_query_at`, `post_at`),
so the second host keeps the deployment check, the pre-request token refresh,
the redacted operation in the error and the single place the status is
inspected.

Its JSON is **camelCase**, like the net-liq service and unlike every other area.

`status` stays `String` and `simulate-trade`'s body is passed through as JSON:
no payload from this service has been captured — the whole area is unreachable
from a checkout with no OAuth grant — and a guessed type would refuse requests
the venue accepts.

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
`list_equity_options()` and `list_future_options()`. Neither appears in the
published spec; keep them.

**`list_equity_options()` is implemented but unverified.** The probe on
2026-08-04 got `403` from `/instruments/equity-options` under a grant reporting
`read,trade,openid`, so nothing has observed this method work. The earlier claim
here that both "work against the venue" was inherited, not measured, and it is
withdrawn. The cause of the refusal is not established —
[#129](https://github.com/joaquinbejar/tastytrade/issues/129).

**They keep returning `Vec<T>`**, unlike every other listing. The `20250715`
release note says they paginate, and the spec published the same day does not
describe them at all — so there is nothing to check the return type against,
and switching them to `Paginated<T>` on the release note alone would make every
existing call fail if the note is wrong. The probe covers `/instruments/equity-options` as one of its controls; it
answered `403`, so this is not settled either.

### The cryptocurrency suspension — [#91](https://github.com/joaquinbejar/tastytrade/issues/91)

tastytrade disabled cryptocurrency **trading** through the API on 2026-06-29,
until further notice ([release notes](https://developer.tastytrade.com/release-notes/)).

Discovery and market data are unaffected: `list_cryptocurrencies`,
`get_cryptocurrency` and the DXLink feed all keep working, and the two endpoints
above are counted as implemented because they are.

Order **routing** for a cryptocurrency leg is refused locally as a non-retryable
`Precondition`, on the placement path, the dry-run path and the complex-order
path alike — a dry run that succeeded while placement refused would be a worse
answer than one consistent refusal.

The whole decision is one constant, `CRYPTOCURRENCY_TRADING_ENABLED`. Restoring
trading is flipping it; no business rule is baked into the order paths that
would be wrong the day the venue changes its mind, and a unit test fails if the
constant and the guard ever disagree. `TastyTrade::post` is public and unguarded
for a caller who finds the venue restored it before this crate did.

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
| `GET /instruments/equity-deliverables` | ❔ routed by the venue, undocumented, access-restricted for a reason not established | 2026-08-04 |
| `GET /instruments/future-spreads` | ❔ routed by the venue, undocumented, access-restricted for a reason not established | 2026-08-04 |

**Legend.** ✅ implemented · ❌ published and not yet implemented · ❔ routed
by the venue but described in no current public API document, so there is no
contract to implement against. The third state is the point of this section: it
is not a backlog item, and it is not an absence either — see the probe result
below for what distinguishes the two.

#### The evidence

The release note stamped `20250715` at <https://developer.tastytrade.com/release-notes/>
says response data is now paginated for eight endpoints, listing
`GET /instruments/equity-deliverables` and `GET /instruments/future-spreads`
among them. So both existed on 2025-07-15.

The Instruments OpenAPI document currently served from
<https://developer.tastytrade.com/open-api-spec/instruments/> is
`instruments-api-swagger_20250715.json` — the **same date** — and contains 24
paths, neither of them among them.

**Nor is any other area.** On 2026-08-04 the sweep was widened from Instruments
to every OpenAPI document the developer site publishes — sixteen areas, **85
paths** — on the chance that the routes had moved rather than gone. They are in
none of them:

| Area | Paths | Area | Paths |
|------|------:|------|------:|
| `account-status` | 1 | `market-sessions` | 11 |
| `accounts-and-customers` | 4 | `net-liquidating-value-history` | 1 |
| `backtesting` | 6 | `orders` | 12 |
| `balances-and-positions` | 4 | `quote-alerts` | 2 |
| `instruments` | 24 | `risk-parameters` | 4 |
| `margin-requirements` | 2 | `symbol-search` | 1 |
| `market-data` | 1 | `transactions` | 3 |
| `market-metrics` | 3 | `watchlists` | 6 |

Reproducible without credentials: each page embeds its document in a
`__NEXT_DATA__` script tag under `props.pageProps.specData[0].spec`, so fetching
the sixteen slugs above from `https://developer.tastytrade.com/open-api-spec/`
and grepping the `paths` keys re-runs this in about a minute.

**And no later release note touches them.** The notes document carries five
dated entries — `20240501`, `20250715`, `20250813`, `20260211`, `20260629`.
Three were published *after* the one that names these routes, and none of the
three mentions instruments, deliverables or spreads at all. So the venue has
neither retired them in writing nor mentioned them again: the 20250715 note
remains the only place either route appears in any published material.

#### Why that is not enough to declare them retired

The same release note names two more endpoints that the same-day spec also
omits: `GET /instruments/equity-options` and `GET /instruments/future-options`,
the plural list forms. Both are present in the earlier spec capture kept at
`Doc/Instruments.json` and both are implemented here, as `list_equity_options()`
and `list_future_options()`.

Four of the eight endpoints in that release note are missing from the spec
published beside it. Absence from the document is therefore evidence about the
**document**, not about the API. Deriving a client contract for `equity-deliverables` or
`future-spreads` from the release note alone would be inventing one — an
`items` envelope and a pagination block is the entire published description,
with no field list, no filters and no response schema.

#### What the venue answered — 2026-08-04, certification

The probe ran. Read-only `GET`s against `api.cert.tastyworks.com`:

| Path | Status | Reading |
|------|--------|---------|
| `/instruments/there-is-no-such-route` | **404** | negative control — the venue 404s a path it does not route |
| `/instruments/equity-deliverables` | **403** | under investigation |
| `/instruments/future-spreads` | **403** | under investigation |
| `/instruments/equity-options` | **403** | control — absent from the spec, called by this crate |
| `/instruments/equities` | **200** | positive control — 24,692 items; offset 1 returns a different record from offset 0 |

**Both routes exist.** The negative control is what makes that readable: this
deployment answers `404` for a path it does not route, and neither route under
investigation answered `404`. `403` is a different answer — the request reached
something and was refused. Absence was the hypothesis this probe was built to
test, and it is now ruled out.

What is *not* established is the contract. A `403` carries no payload, so there
is still no field list, no filters and no response schema — the same position as
before on that question, reached from evidence rather than from a document's
silence. Nothing here can be implemented yet, and implementing it from the
release note would still be inventing it.

The `equity-options` row is why the refusal is not read as a statement about
these two routes in particular: that route is called by this crate and is
refused under the same grant.

**What the refusal is not.** It is not a missing read scope. The grant reports
`read,trade,openid` and `/instruments/equities` answers `200` with that same
token, so an ordinary read succeeds beside the refusals. The cause is not
established — [#129](https://github.com/joaquinbejar/tastytrade/issues/129)
investigates it — and describing it as an entitlement or scope problem would be
asserting something the evidence does not support.

**Production is unprobed.** The refresh token in this checkout is a
certification grant: `POST /oauth/token` against production answers `400`, so
the exchange fails before any instrument route is reached. The determination
above is therefore certification-only. It is the weaker of the two — a route
certification serves is served, but a route it refuses might still be entitled
in production.

#### How it was settled

One read-only GET per route against a live host. That is
`examples/instruments/src/bin/probe_undocumented.rs`, which probes both routes
plus the two controls above and reports the status and envelope shape:

```shell
TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin probe_undocumented
```

Run it **twice**, once per host, because the two answers are not
interchangeable — a route certification does not serve is not a route
production has retired:

```shell
TASTYTRADE_USE_DEMO=true  cargo run -p instruments --bin probe_undocumented
TASTYTRADE_USE_DEMO=false cargo run -p instruments --bin probe_undocumented
```

The second line reads production. It is read-only — the probe issues `GET` and
nothing else — but it is still production, so it is a deliberate act and the
crate makes it one: anything other than the literal `false` resolves to
certification.

### What a certification grant can actually read — [#125](https://github.com/joaquinbejar/tastytrade/issues/125)

Whether a fixture can be captured is a property of the **grant**, not of this
crate, and it differs per route. Measured on 2026-08-04 against certification
with `examples/instruments/src/bin/probe_entitlements.rs`, read-only:

| Answer | Routes |
|--------|--------|
| **200** — capturable today | `/customers/me`, `/customers/me/accounts`, `/instruments/equities`, `/instruments/cryptocurrencies`, `/instruments/warrants`, `/instruments/futures`, `/instruments/future-products`, `/instruments/quantity-decimal-precisions`, `/option-chains/{symbol}/nested`, `/instruments/search`, `/market-time/equities/sessions/current` |
| **403** — routed, access-restricted, cause unknown | `/instruments/equity-options` |
| **502** — **not served by certification** | `/symbols/search/{symbol}`, `/market-metrics`, `/market-data/by-type`, `/quote-alerts`, `/watchlists`, `/pairs-watchlists` |

Eleven of eighteen. The `502` group is the one that matters for planning:
certification does not serve those services at all, which the venue's own
sandbox page already says about Market Metrics. Their fixtures cannot come from
certification, and no change to the grant alters that.

#### Captured, 2026-08-04

Ten of the eleven reachable families were captured and the serde tests now read
them (`tests/integration/captures.rs`). Reproduce with:

```shell
TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin capture_fixtures
```

Redaction happens before the bytes reach the disk, in three tiers: instruments,
chains and sessions are stored as they arrived; the account listing has its
number and nickname replaced; the customer resource keeps its shape and has
**every leaf value** replaced, because the whole record is personal and a
field-by-field rule over 171 fields is a field-by-field chance to miss one.

`/instruments/warrants` was **not** captured: certification holds none, and an
empty listing pins down no field of the type it is meant to check. Its fixture
stays hand-written and says so.

**What the captures found, which is the reason for doing this:** the customer
resource did not decode at all. Six fields were typed `Option<String>` where the
venue sends booleans, and `permitted-account-types` was typed `String` where the
venue sends an array of objects — so `TastyTrade::customer()` failed against the
real API while every hand-written test passed. Two account fields were also
asserted with values nobody had ever received.

#### The value sets that were still guesses

Four fields were `String` because no captured payload showed their values.
Measured with `examples/instruments/src/bin/probe_enum_values.rs`:

| Field | Records read | Values observed | Outcome |
|-------|-------------:|-----------------|---------|
| `product-type` | 83 products, 305 occurrences | `Financial` (82), `Physical` (223) | **narrowed** to `ProductType` |
| `margin-or-cash` | 1 account | `Margin` | still `String` — one record is not a set |
| `option-chain-type` | 1 chain | `Standard` | still `String` — one observation is not a set |
| `option-type` | 1 chain | *field absent* | still `String` — carried by the equity options listing, which is `403` |

`FutureOptionProduct::product_type` documented its example value as
`"future option"`. No record carries that: all 305 occurrences across the
listing, nested option products included, are `Financial` or `Physical`. The
doc was wrong and is corrected.

#### Enums that were derived, now confirmed against real payloads

This is what #125 is for. These were tolerant enums whose variants came from
fixtures written from the same document the types came from — agreeing with
themselves. The census read them from the venue:

| Enum | Observed |
|------|----------|
| `Lendability` | `Easy To Borrow` (235), `Preborrow` (765) over 1,000 equities. `Locate Required` unobserved. |
| `ExpirationType` | `Regular` (13), `Weekly` (17), `Quarterly` (4) over 34 expirations. |
| `SettlementType` | `PM` (34). `AM` unobserved. |

No contradiction in any of them. Unobserved variants are not wrong — they are
unobserved, and the `Unknown` arm is why that costs nothing either way.

## Margin Requirements

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /margin/accounts/{account_number}/requirements` | `margin_requirements()` | ✅ |
| `POST /margin/accounts/{account_number}/dry-run` | `estimate_margin()` | ✅ |

`estimate_margin` routes nothing and is named to stay apart from
`Account::dry_run`, the order preflight against `/accounts/{n}/orders/dry-run`.
It takes `MarginOrderRequest`, which carries the account number and underlying
symbol an `Order` does not — so an `Order` cannot be handed to it by accident,
and there is no path from here to a placement.

One to four unique legs, checked locally as a non-retryable `Precondition`. A
repeated leg is almost always one leg written twice, and a doubled requirement
is the kind of wrong that looks plausible.

## Market Data

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /market-data/by-type` | `market_data_by_type()` | ✅ |

Snapshot quotes over REST for up to 100 symbols, without opening a websocket.

The encoding is unlike everything else here: **one parameter per instrument
type**, each a comma-separated list — `equity=AAPL,TSLA&cryptocurrency=BTC/USD`
— rather than the repeated keys the instrument listings use. Getting it backwards
returns one symbol per type and looks like thin data rather than a client bug.

The 100-symbol cap counts **every type together**, which is the part a caller
building one watchlist per type gets wrong. Over it is a non-retryable
`Precondition`, refused before anything is sent.

Every price is `Decimal`. The `f64` exemption is for the DXFeed streaming types,
where the feed imposes it; these are different types with a different field set.

The venue sends `-1` for `halt-start-time` when nothing is halted. That is a
sentinel, not a time, and it stays the integer it is rather than becoming a
timestamp in 1969.

## Market Metrics

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /market-metrics` | `market_metrics()` | ✅ |
| `GET /market-metrics/historic-corporate-events/dividends/{symbol}` | `historic_dividends()` | ✅ |
| `GET /market-metrics/historic-corporate-events/earnings-reports/{symbol}` | `historic_earnings()` | ✅ |

IV index, IV rank, IV percentile, liquidity and its rank and rating, plus the
per-expiration volatility block and the dividend and earnings histories.

`symbols` is **comma-joined into one parameter**, not the repeated keys the
instrument listings use. The venue documents it that way, and getting it wrong
returns metrics for one symbol — which reads as a thin answer rather than a
client bug.

`EarningsRange` carries the `start-date` the venue marks required, so it cannot
be omitted.

An option expiration decodes as a **calendar day** even though the schema types
it `date-time`: an expiration is a day of market and there is no timezone to
invent. Both shapes decode, and anything else is still an error.

Every numeric field is `Decimal`. IV also exists as `f64` in
`types::dxfeed`, but that exemption belongs to the streaming types where the
feed imposes it.

**Live only** per the venue's sandbox page, so all three examples require an
explicit read-only production opt-in.

## Market Sessions

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /market-time/sessions` | `market_sessions()` | ✅ |
| `GET /market-time/sessions/current` | `current_market_session()` | ✅ |
| `GET /market-time/equities/sessions/current` | `current_equities_session()` | ✅ |
| `GET /market-time/equities/sessions/next` | `next_equities_session()` | ✅ |
| `GET /market-time/equities/sessions/previous` | `previous_equities_session()` | ✅ |
| `GET /market-time/equities/holidays` | `equities_holidays()` | ✅ |
| `GET /market-time/futures/sessions/current` | `current_futures_sessions()` | ✅ |
| `GET /market-time/futures/sessions/current/{instrument_collection}` | `current_futures_session()` | ✅ |
| `GET /market-time/futures/sessions/next/{instrument_collection}` | `next_futures_session()` | ✅ |
| `GET /market-time/futures/sessions/previous/{instrument_collection}` | `previous_futures_session()` | ✅ |
| `GET /market-time/futures/holidays/{instrument_collection}` | `futures_holidays()` | ✅ |

Session boundaries keep the **offset the venue sent** — going to UTC is one-way,
and a market open shown in the wrong zone is wrong. Holidays are `NaiveDate`,
because a holiday is a day.

`InstrumentCollection` is a `wire_enum!` over the three values the schema
enumerates (`CFE`, `CME`, `Equity`), modelled as a type rather than a bare
string because eleven endpoints take one and a typo in any of them is a 404. The
futures family carries it in the **path**, so it goes through the shared
encoder.

`to-date` is a constructor argument on `SessionRange` because the venue marks it
required, and a range longer than nine months — or an inverted one — is refused
locally with the limit named in the message.
`instrument-collections[]` is required too, so `current_market_session` takes a
first collection separately from the rest: an empty selection is
unrepresentable.

`is_open_at` is derived from the fetched session and takes the moment as an
argument, so it stays a pure function and never consults a local clock about an
exchange's timezone. It answers `None` when the venue did not send both
boundaries: "we were not told" is not "closed".

`market-holidays` and `market-half-days` are typed `object` with no properties
in the schema, which is a generation artifact — the same document types a
decimal quantity that way. They are read as arrays of calendar days.

## Net Liquidating Value History

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{accountNumber}/net-liq/history` | `net_liq_history()` | ✅ |

Two things about this endpoint are unlike the rest of the API. It is served by a
different system — OpenAPI 3 from a JVM service, where every other area is
Swagger 2 — and that system spells its JSON in **camelCase** rather than the
kebab-case everything else uses. `NetLiqOhlc` accepts both, because the two have
disagreed before and a chart that silently comes back empty is worse than one
that fails.

`time` stays `String`. Its schema gives it no format, and the same service
documents JVM `ZonedDateTime` for its inputs —
`2011-12-03T10:15:30+01:00[Europe/Paris]`, which is not RFC 3339 and which
`chrono` does not produce. `start-time` and `end-time` are `String` for the same
reason.

`time-back` and an explicit window are one enum, so a request carrying both
cannot be built. `interval` stays `String`: the schema declares it with no
enumerated values, unlike `time-back` beside it.

**Live only** per the venue's sandbox page. The example requires an explicit
read-only production opt-in and never places or modifies anything.

## Orders

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/orders` | `search_orders()` | ✅ |
| `POST /accounts/{account_number}/orders` | `place_order()` / `place_reviewed_order()` | ✅ |
| `POST /accounts/{account_number}/orders/dry-run` | `dry_run()` / `review_order()` | ✅ |
| `GET /accounts/{account_number}/orders/live` | `live_orders()` / `live_orders_matching()` | ✅ |
| `GET /accounts/{account_number}/orders/{id}` | `order()` | ✅ |
| `PUT /accounts/{account_number}/orders/{id}` | `place_reviewed_amendment()` (Replace) | ✅ |
| `PATCH /accounts/{account_number}/orders/{id}` | `place_reviewed_amendment()` (Edit) | ✅ |
| `DELETE /accounts/{account_number}/orders/{id}` | `cancel_order()` | ✅ |
| `POST /accounts/{account_number}/orders/{id}/dry-run` | `review_amendment()` | ✅ |
| `GET /accounts/{account_number}/complex-orders` | `complex_orders()` | ✅ |
| `POST /accounts/{account_number}/complex-orders` | `place_reviewed_complex_order()` | ✅ |
| `POST /accounts/{account_number}/complex-orders/dry-run` | `review_complex_order()` | ✅ |
| `GET /accounts/{account_number}/complex-orders/live` | `live_complex_orders()` | ✅ |
| `GET /accounts/{account_number}/complex-orders/{id}` | `complex_order()` | ✅ |
| `PATCH /accounts/{account_number}/complex-orders/{id}` | `place_reviewed_pairs_threshold()` | ✅ |
| `DELETE /accounts/{account_number}/complex-orders/{id}` | `cancel_complex_order()` | ✅ |
| `POST /accounts/{account_number}/complex-orders/{id}/dry-run` | `review_pairs_threshold()` | ✅ |
| `GET /customers/{customer_id}/orders` | `customer_orders()` | ✅ |
| `GET /customers/{customer_id}/orders/live` | `customer_live_orders()` | ✅ |

The single-order lifecycle is complete. Amending goes through the same
discipline as placing: `review_amendment` → read the warnings → `accept()` →
`place_reviewed_amendment`. The intent — Replace or Edit — is recorded **at
review time** and decides the verb, so an amendment reviewed as one cannot be
applied as the other; the venue treats them differently and a caller should not
be able to swap them after reading the answer. The receipt binds the account
**and** the `base_url`, because certification reuses production numbering, and
it is not `Clone`.

A replacement is not atomic at the venue — a fill on the original aborts it —
and this crate does not paper over that.

`OrderFilter` and `LiveOrderFilter` are different types because the two routes
take different parameters: the history one repeats `status[]`, the live one
takes a **single** `status`. Sending the history filters to the live endpoint
would be ignored, and the caller would believe a full listing had been narrowed.

`OrderStatus` is now a `wire_enum!` with an `Unknown(String)` arm. It is a
response enum and `Items<T>` skips what it cannot parse, so a status the venue
adds later would have made the order carrying it vanish from a live-orders
listing without an error. `is_terminal()` answers `false` for an unknown value:
a status this crate has not seen says nothing about whether the order is done.

Complex orders go through the same receipt discipline, for the same reason:
they route real money. Local checks run first — an OCO with one component is not
an OCO, and a PAIRS trade with no threshold has no trigger — so neither reaches
the venue. `minimum_components` matches on `ComplexOrderType` **exhaustively
with no wildcard**, so a strategy the venue adds later breaks the build rather
than inheriting whichever default a `_` arm gave it.

`PATCH /complex-orders/{id}` changes only the threshold price of a PAIRS trade,
which is narrower than the plain-order patch, so it has its own type and its own
receipt.

Complex-order identifiers are **strings**, not the `u64` a plain order carries,
and they go through the shared path encoder.

## Quote Alerts

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /quote-alerts` | `quote_alerts()` | ✅ |
| `POST /quote-alerts` | `create_quote_alert()` | ✅ |
| `DELETE /quote-alerts/{alert_external_id}` | `cancel_quote_alert()` | ✅ |

The two halves share one type. `AccountStreamer` already delivered `QuoteAlert`
through `quote-alerts-subscribe`; the REST side returns the **same** struct, so a
caller cannot set an alert with one shape and receive it as another.

Alerts are per **user**, not per account, so they hang off `TastyTrade` rather
than `Account` — putting them on an account would imply a scoping the venue does
not have.

`QuoteAlertField` and `QuoteAlertOperator` are `wire_enum!`s over the values the
create body enumerates (`Last`, `Bid`, `Ask`, `IV`; `>` and `<`). They keep an
`Unknown` arm because the same types appear on the **read** side, where a value
the venue adds later would otherwise make the alert carrying it vanish through
`Items<T>`.

`NewQuoteAlert::new` takes the threshold once and renders it into both wire
forms, so they cannot disagree — a threshold that disagrees with itself is an
alert that fires at the wrong price. A threshold of zero is refused locally:
it would fire on the first quote, which is almost always a caller who forgot to
set one.

## Risk Parameters

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /accounts/{account_number}/margin-requirements/{underlying_symbol}/effective` | `effective_margin_requirement()` | ✅ |
| `GET /accounts/{account_number}/position-limit` | `position_limit()` | ✅ |
| `GET /margin-requirements-public-configuration` | `margin_requirements_configuration()` | ✅ |
| `GET /span/rows` | `span_rows()` | ✅ |

`span_rows` takes `date` and `exchange` as arguments because the venue marks
both required — a required query parameter should be impossible to omit rather
than a runtime 400. `row_data` stays text: a SPAN row is a fixed-width record in
the exchange's own format, and parsing it is a different job from talking to
this API.

The margin report sends the literal string `NaN` for a fixing price that does
not apply. `Decimal` is fixed-point and has no such value, so it decodes as
`None` — without that the whole report fails to decode. Only `NaN` is forgiven;
anything else unparseable is still an error.

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

| Endpoint | Method | Status |
|----------|--------|--------|
| `GET /public-watchlists` | `public_watchlists()` / `public_watchlist_counts()` | ✅ |
| `GET /public-watchlists/{watchlist_name}` | `public_watchlist()` | ✅ |
| `GET /watchlists` | `watchlists()` | ✅ |
| `POST /watchlists` | `create_watchlist()` | ✅ |
| `GET /watchlists/{watchlist_name}` | `watchlist()` | ✅ |
| `PUT /watchlists/{watchlist_name}` | `replace_watchlist()` | ✅ |
| `DELETE /watchlists/{watchlist_name}` | `delete_watchlist()` | ✅ |
| `GET /pairs-watchlists` | `pairs_watchlists()` | ✅ |
| `GET /pairs-watchlists/{pairs_watchlist_name}` | `pairs_watchlist()` | ✅ |

The only user-owned mutable resource besides orders, and the only area where a
client can **destroy** user data.

`replace_watchlist` replaces **every property**. It is not an append and not a
merge: the entries sent are the entries that survive. `delete_watchlist` is
irreversible and takes the name explicitly, so it cannot be reached from a
listing or a read by accident.

`NewWatchlist` is separate from `Watchlist` because the create body is not the
read shape — it has no `cms-id`, and sending one as `null` is a different
request from not sending it. A blank name is refused locally: the name is also
the URL segment a later replace or delete addresses, so a list nobody can name
is a list nobody can remove.

`Watchlist` and `WatchlistEntry` are the same types
`public-watchlists-subscribe` delivers on the account websocket.

`pairs-equations` stays `Vec<Value>`: the venue's schema types it `object` with
no properties, so there is nothing to model against.

The three mutating examples run against certification only, on uniquely named
throwaway lists, and clean up after themselves.

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
