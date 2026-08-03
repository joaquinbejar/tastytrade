[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Downloads](https://img.shields.io/crates/d/tastytrade.svg)](https://crates.io/crates/tastytrade)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/stargazers)
[![Issues](https://img.shields.io/github/issues/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/issues)
[![PRs](https://img.shields.io/github/issues-pr/joaquinbejar/tastytrade.svg)](https://github.com/joaquinbejar/tastytrade/pulls)
[![Tests](https://img.shields.io/github/actions/workflow/status/joaquinbejar/tastytrade/tests.yml?branch=main&label=tests)](https://github.com/joaquinbejar/tastytrade/actions/workflows/tests.yml)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/tastytrade)](https://codecov.io/gh/joaquinbejar/tastytrade)
[![Dependencies](https://img.shields.io/librariesio/github/joaquinbejar/tastytrade)](https://libraries.io/github/joaquinbejar/tastytrade)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/tastytrade)
[![Wiki](https://img.shields.io/badge/wiki-latest-blue.svg)](https://deepwiki.com/joaquinbejar/tastytrade)

# tastytrade

A Rust client for the [tastytrade](https://developer.tastytrade.com) brokerage
API: accounts, balances, positions, transactions, instruments, option chains,
market data, orders — and two real-time websockets.

**Orders placed through this crate move real money.** Certification is the
default and production is a deliberate opt-in; see
[Certification is the default](#certification-is-the-default).

## Install

```toml
[dependencies]
tastytrade = "0.4"
```

Minimum supported Rust version: **1.88**. It is declared as `rust-version` in
`Cargo.toml` and a CI job builds the library against exactly that toolchain, so
the two cannot drift.

## Authentication

tastytrade **decommissioned `POST /sessions` on 2026-02-11**. Username and
password authentication, session tokens and remember tokens are gone from the
venue, and gone from this crate with them. If you have arrived from an older
version or an older tutorial looking for `login()`, this is why it is not here —
and there is no deprecated shim, because a deprecated `login()` would still call
an endpoint that no longer exists.

OAuth2 is the only flow, in two documented grants.

### The personal refresh-token grant

Create an OAuth application and a personal grant under **Manage → My Profile →
API** on [my.tastytrade.com](https://my.tastytrade.com). That gives you a client
secret and a refresh token.

```shell
# .env
TASTYTRADE_CLIENT_SECRET=...
TASTYTRADE_REFRESH_TOKEN=...
TASTYTRADE_USE_DEMO=true
```

```rust,no_run
use tastytrade::TastyTrade;
use tastytrade::utils::config::TastyTradeConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TastyTradeConfig::from_env();
    let tasty = TastyTrade::connect(&config).await?;

    for account in tasty.accounts().await? {
        println!("{}", account.number().redacted());
    }
    Ok(())
}
```

Access tokens last about fifteen minutes. You do not have to manage that: every
request renews the token in hand before it expires. A renewal is never a
*retry* — a `POST` that may have placed an order is not replayed on a `401`.

### The third-party authorization-code grant

For an application acting on somebody else's account. Send the customer to the
authorization page, then exchange the code:

```rust,no_run
use tastytrade::TastyTrade;
use tastytrade::oauth::{AuthorizationRequest, Scope};
use tastytrade::utils::config::TastyTradeConfig;

# async fn authorize(state: &str, code: String, returned_state: Option<&str>)
# -> Result<(), Box<dyn std::error::Error>> {
let config = TastyTradeConfig::from_env();

let request = AuthorizationRequest::new(&config.client_id, &config.redirect_uri)
    .with_scopes([Scope::Read, Scope::Trade])
    .with_state(state);

println!("send the customer to {}", request.authorize_url(config.environment())?);

// The `state` must come back unchanged before anything is exchanged. This crate
// does not generate one: a library-generated nonce the application cannot
// correlate is a nonce that proves nothing.
if returned_state != Some(state) {
    return Err("the authorization state did not come back".into());
}

let tasty = TastyTrade::connect_with_authorization_code(&config, code).await?;
# let _ = tasty;
# Ok(())
# }
```

## Certification is the default

`TastyTradeConfig::from_env` selects **certification**
(`api.cert.tastyworks.com`). Production takes a literal opt-in:

```shell
TASTYTRADE_USE_DEMO=false   # production — orders placed here are real
```

Only a value that parses as `false` selects production. A missing, empty or
misspelled variable resolves to certification, so a typo cannot be what points an
order at a funded account. Connecting without credentials fails locally and never
reaches the network.

A session is bound to the deployment it authenticated against: it will not
present a certification token to production, and it will not send the client
secret to a host it did not authenticate with.

## What it covers

Every REST endpoint published in tastytrade's OpenAPI documents — **97 of 97**.
`Doc/API_Coverage_Status.md` is the endpoint-by-endpoint matrix.

### Accounts

Balances (one row per currency, plus a per-currency lookup), balance snapshots,
filtered positions, trading status, transactions, net-liq history, margin
requirements and risk parameters, and the customer resource.

```rust,no_run
# use tastytrade::prelude::*;
# async fn money(account: &Account<'_>) -> Result<(), Box<dyn std::error::Error>> {
for row in account.balances().await? {
    println!("{:?}: {}", row.currency, row.cash_balance);
}

// Filtered at the venue, not downloaded and filtered here.
let held = account
    .positions_matching(&PositionFilter::new().with_marks(true))
    .await?;

// The ledger: fills, fees, dividends, assignments, cash movements.
let ledger = account
    .transactions(&TransactionFilter::new().with_page(PageRequest::first()))
    .await?;

// The cheap check before an order.
let status = account.trading_status().await?;
println!("blocked: {}", status.is_blocked());
# let _ = (held, ledger);
# Ok(())
# }
```

The customer resource carries names, addresses, tax identifiers and birth dates.
**Nothing in it renders itself** — `Debug` and `Display` print a field count, and
reading a value means naming the field.

### Instruments and option chains

Equities, equity options, futures, future options, their products,
cryptocurrencies, warrants, flat/compact/nested chains, and symbol and instrument
search. The listings paginate and take typed filters.

```rust,no_run
# use tastytrade::prelude::*;
# async fn browse(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
let mut filter = EquityFilter::new()
    .with_is_etf(true)
    .with_lendability(Lendability::EasyToBorrow)
    .with_page(PageRequest::first().with_per_page(25));

loop {
    let page = tasty.list_equities(&filter).await?;
    for equity in &page {
        println!("{} — {}", equity.symbol.0, equity.description);
    }
    // Offsets count from zero, so the last page is `total_pages - 1`.
    if !page.has_more() {
        break;
    }
    let next = filter.page().next_page();
    filter = filter.with_page(next);
}
# Ok(())
# }
```

### Market data

REST snapshots for up to 100 symbols without opening a websocket, market metrics
(IV index, rank and percentile, liquidity), dividend and earnings history, and
market sessions and holidays.

```rust,no_run
# use tastytrade::prelude::*;
# async fn prices(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
let snapshot = tasty
    .market_data_by_type(
        &MarketDataRequest::new()
            .with_equities(&["AAPL", "TSLA"])
            .with_cryptocurrencies(&["BTC/USD"]),
    )
    .await?;

// Never hardcode an exchange calendar; it is wrong roughly once a quarter.
let session = tasty
    .current_market_session(SessionCollection::Equity, &[])
    .await?;
# let _ = (snapshot, session);
# Ok(())
# }
```

Market Metrics and Net Liq History are **live only** per the venue's sandbox
page; their examples require an explicit read-only production opt-in.

### Orders

The reviewed placement flow is *the* documented path:

```rust,no_run
# use tastytrade::prelude::*;
# async fn place(account: &Account<'_>, order: &Order)
# -> Result<(), Box<dyn std::error::Error>> {
let receipt = account.review_order(order).await?;

for warning in receipt.warnings() {
    println!("{warning}");
}

// `accept` refuses when the venue attached warnings. Not a refusal to proceed —
// a refusal to proceed *silently*. `accept_with_warnings` is the deliberate
// alternative, and its name says so.
let reviewed = receipt.accept()?;
account.place_reviewed_order(reviewed).await?;
# Ok(())
# }
```

**Why the receipt exists:** it binds the account number **and** the `base_url`.
Certification reuses production account numbering, so without the origin a
sandbox dry run would authorise a real order against the same number. No receipt
is `Clone` — duplicable proof is not proof.

The same shape covers cancel-replace and editing
(`review_amendment` → `place_reviewed_amendment`) and complex orders — OCO, OTOCO,
PAIRS — (`review_complex_order` → `place_reviewed_complex_order`). Search, fetch
by id, and cancel are there too, along with the customer-scoped order searches.

A replacement is **not atomic at the venue**: a fill on the original aborts it.
This crate does not paper over that.

### Watchlists and quote alerts

Watchlists are the only user-owned mutable resource besides orders — and the only
place this crate can destroy user data. `replace_watchlist` replaces **every
property**: the entries sent are the entries that survive.

Quote alerts are set over REST and fire over the account websocket, using the
same `QuoteAlert` type on both sides.

### Backtesting

Server-side strategy backtests, on their own host
(`https://backtester.vast.tastyworks.com`). Asynchronous: create, poll, read
logs, cancel. The polling is yours — how long to wait is not the library's
decision.

## Streaming

Two websockets, and they are different services.

**Market data** is DXLink, reached with a token from `GET /api-quote-tokens`. All
eleven event types are routed: quotes, regular and extended-hours trade prints
(`TradeETH`), Greeks, candles, summaries, time and sale, profiles, underlyings,
theoretical prices and series.

```rust,no_run
# use chrono::{Duration, Utc};
# use tastytrade::{Symbol, TastyTrade};
# use tastytrade::dxfeed::{CandlePeriod, EventData, EventKind};
# async fn bars(tasty: &TastyTrade) -> Result<(), Box<dyn std::error::Error>> {
let mut streamer = tasty.create_quote_streamer().await?;
let mut bars = streamer.create_sub([EventKind::Candle]).await?;

// Candles are the only route to a price series in this crate. `from_time` is
// required: without one, a candle subscription replays an unbounded history.
bars.add_candles(
    &[Symbol("AAPL".to_string())],
    CandlePeriod::minutes(5)?,
    Utc::now() - Duration::days(2),
)
.await?;
# Ok(())
# }
```

**Account notifications** come over tastytrade's own streamer, authenticated with
the access token. It publishes a full object on every change — never a diff — for
orders, balances, positions, quote alerts and public watchlists. The fills inside
an order's legs are the only place an executed price reaches this crate.

**Reconnection.** Both sides reconnect under a `BackoffPolicy` and replay what
was subscribed. `state()` reports a `ConnectionState`, and `Connected` is claimed
only once the subscriptions are restored and the venue has acknowledged them. The
attempt budget resets only on evidence the venue accepted something, so a host
that takes the socket and then refuses the session runs out of attempts instead of
retrying forever.

A subscription's buffer is bounded, so a slow consumer loses events rather than
stalling every other subscription. `lagged()` makes that observable, and for
candles a dropped bar is recoverable across a reconnect.

## The cryptocurrency trading suspension

tastytrade **disabled cryptocurrency trading through the API on 2026-06-29**,
until further notice
([release notes](https://developer.tastytrade.com/release-notes/)). An order with
a cryptocurrency leg is refused locally on the placement, dry-run and
complex-order paths alike.

**Instrument discovery and market data are unaffected.**
`list_cryptocurrencies`, `get_cryptocurrency` and the DXLink feed all keep
working; only routing is closed. The whole decision is one constant,
`CRYPTOCURRENCY_TRADING_ENABLED`.

## Design decisions you can feel

- **Money is `Decimal`.** Every price, quantity, balance and ratio is
  `rust_decimal::Decimal`. `f64` appears in exactly one place — the DXFeed
  streaming types, where the feed imposes it — and REST paths never reuse those
  types even where the field names match.
- **Secrets never render themselves.** The client secret, the refresh and access
  tokens, the DXLink token, the AI-search token and the whole customer resource
  print as `***` or as a field count — not in `Debug`, not in `Display`, not in a
  log, not in an error message. Account numbers are redacted from request paths
  in errors, and a response body is never logged at any level.
- **A library does not panic.** No `unwrap`, no `expect`, no unchecked indexing
  on a path reachable from a public method. A local failure is `Precondition` and
  reports `is_retryable()` false, because nothing was sent.
- **An absent field is unknown, never zero.** A flag the venue did not send is
  `None`, not `false`. Certification omits fields production sends.
- **`Items<T>` tolerates one bad row** rather than losing a listing of 5,000 — so
  response enums keep an `Unknown(String)` arm, because a strict one would make a
  row disappear silently. Request enums are closed, for the opposite reason.

## Examples

Six runnable example crates in the workspace:

| Crate | What it shows |
|---|---|
| `examples/account-data` | Customer, transactions, trading status, margin, net-liq history |
| `examples/accounts-status` | Balances, positions, account streaming, frame capture |
| `examples/instruments` | Equities, futures, options, chains, search |
| `examples/market-data` | REST snapshots, metrics, sessions, watchlists, quote alerts, backtesting |
| `examples/orders` | Order search, replace, edit, complex orders |
| `examples/quote-streaming` | DXLink quotes, greeks, candles |

```shell
cp .env.example .env      # then fill in the OAuth credentials
TASTYTRADE_USE_DEMO=true cargo run -p instruments --bin test_equities
```

Anything that mutates state refuses to run outside certification. Anything
live-only requires an explicit read-only production opt-in.

## Development

```shell
make check      # the pre-push gate: fmt, clippy, tests, docs — all read-only
make test
make lint
make doc
make coverage
```

`make check` must be green before pushing. If the public surface moved,
`cargo semver-checks check-release` too.

### **Contact Information**

- **Author**: Joaquín Béjar García
- **Email**: <jb@taunais.com>
- **Telegram**: [@joaquin_bejar](https://t.me/joaquin_bejar)
- **Repository**: <https://github.com/joaquinbejar/tastytrade>
- **Crate**: <https://crates.io/crates/tastytrade>
- **Documentation**: <https://docs.rs/tastytrade>

## Contribution

We welcome contributions to this project! If you would like to contribute,
please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

## License

Licensed under the MIT license. See [LICENSE](./LICENSE).
