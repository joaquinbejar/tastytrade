
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


## tastytrade

`tastytrade` is a Rust client library for the Tastytrade API, providing programmatic access to
trading functionality, market data, and account information.

### Features

- OAuth2 authentication, with automatic access-token renewal
- Real-time market data streaming via DxFeed
- Account and positions information
- Order management (placing, modifying, canceling)
- Real-time account streaming for balance updates and order status changes

### Authentication

tastytrade **decommissioned `POST /sessions` on 2026-02-11**. Username and
password authentication, session tokens and remember tokens are gone from
the venue, and gone from this crate with it; OAuth2 is the only flow that
works.

Create an OAuth application and a personal grant under Manage → My Profile
→ API on [my.tastytrade.com](https://my.tastytrade.com). That gives you a
**client secret** and a **refresh token**, which are what
[`utils::config::TastyTradeConfig`] reads from
`TASTYTRADE_CLIENT_SECRET` and `TASTYTRADE_REFRESH_TOKEN`.

Access tokens last about fifteen minutes. You do not have to manage that:
every request renews the token first when the one in hand is about to
expire, so a long-lived client keeps working. A renewal is never a *retry* —
a `POST` that may have placed an order is not replayed on a `401`.

### Usage

```rust
use tastytrade::TastyTrade;
use tastytrade::utils::config::TastyTradeConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Authenticate with the personal refresh-token grant

    let config = TastyTradeConfig::from_env();
    let tasty = TastyTrade::connect(&config).await?;

    // Get account information
    let accounts = tasty.accounts().await?;
    for account in accounts {
        // Redacted: doc examples get copied, and an account number in a
        // log is the thing this crate spends most of its care avoiding.
        println!("Account: {}", account.number().redacted());

        // Get positions
        let positions = account.positions().await?;
        println!("Positions: {}", positions.len());
    }

    Ok(())
}
```

### The customer resource

[`TastyTrade::customer`] returns names, addresses, tax identifiers, birth
dates, net worth and employment. It is the most sensitive object this crate
touches, so **nothing in it renders itself**: `Debug` and `Display` print
the type and a field count, never a value.

```rust
let customer = tasty.customer().await?;

// Safe: renders as `Customer(<redacted, N field(s) present>)`.
println!("{customer:?}");

// Reading a value means naming the field, which is a decision rather than
// an accident — and one a reviewer can grep for.
if let Some(country) = customer.citizenship_country.as_deref() {
    println!("citizenship: {country}");
}
```

A customer that may not exist is [`TastyTrade::find_customer`], which sends
the venue's `allow-missing` and answers `Ok(None)` instead of a `404`.

One account is one request. [`TastyTrade::account`] used to download every
account and filter locally, so a *sibling* account that failed to
deserialize took the one you asked for with it — `Items<T>` skips what it
cannot parse, and the answer came back `Ok(None)`, indistinguishable from
"this session cannot see that account". Now `Ok(None)` means the venue
returned `404`.

### Balances and positions

`GET /balances` answers with a **list** — one row per currency the account
holds — and has since 2024-05-01. [`accounts::Account::balance`] is the
shortcut for the single-row case and refuses rather than picking a currency
for you when there is more than one.

```rust
for row in account.balances().await? {
    println!("{:?}: {}", row.currency, row.cash_balance);
}
let usd = account.balance_in("USD").await?;
```

Snapshots take a single day **or** a date range, never both — they are one
enum, so the contradictory query cannot be written. `time-of-day` is
required by the venue and is therefore a constructor argument:

```rust
let page = account
    .balance_snapshots(
        &BalanceSnapshotFilter::at(SnapshotTimeOfDay::Eod)
            .with_range(SnapshotRange::between(from, to))
            .with_page(PageRequest::first().with_per_page(30)),
    )
    .await?;
```

Positions filter at the venue. `positions()` is still every open position;
`positions_matching` narrows the request itself rather than the result:

```rust
let held = account
    .positions_matching(
        &PositionFilter::new()
            .with_underlying_symbols(&["AAPL", "SPY"])
            .with_marks(true),
    )
    .await?;
```

`with_marks` is what fills in [`FullPosition::mark`]; without it the field
is `None` because the venue did not send one, which is not the same as a
mark of zero.

#### The order lifecycle

Placing and cancelling were already here; reading an order back, searching
the history, and amending a working order were not. Cancel-replace is how a
resting order gets repriced — cancel-then-place loses queue position and
leaves the account exposed in between.

```rust
let history = account
    .search_orders(
        &OrderFilter::new()
            .with_statuses(&[OrderStatus::Filled, OrderStatus::Cancelled])
            .with_page(PageRequest::first().with_per_page(50)),
    )
    .await?;

// The live endpoint takes a **single** status, not an array — which is why
// its filter is a different type. Sending the history filters there would
// be ignored, and the listing would look narrowed when it was not.
let working = account
    .live_orders_matching(&LiveOrderFilter::new().with_status(OrderStatus::Live))
    .await?;
```

Amending goes through the same discipline as placing: dry-run, read the
answer, then apply the receipt.

```rust
let amendment = OrderAmendment::new(
    OrderType::Limit,
    TimeInForce::Day,
    Decimal::ZERO,
    PriceEffect::Debit,
    PriceEffect::Debit,
)
.with_price(price);

let receipt = account
    .review_amendment(id, AmendmentIntent::Replace, &amendment)
    .await?;
for warning in receipt.warnings() {
    println!("{warning}");
}
let replaced = account.place_reviewed_amendment(receipt.accept()?).await?;
```

The intent is recorded **at review time**, so an amendment reviewed as a
replacement cannot be applied as an edit: the venue treats them differently
and a caller should not be able to swap them after reading the answer. The
receipt is bound to the account **and** the deployment, because
certification reuses production account numbering.

A replacement is not atomic at the venue — a fill on the original aborts it
— and this crate does not paper over that.

[`prelude::OrderStatus`] keeps an unrecognised value verbatim. `Items<T>`
skips what it cannot parse, so a strict enum would make an order carrying a
new status vanish from a listing without an error, and
[`prelude::OrderStatus::is_terminal`] answers `false` for it: a status this
crate has not seen says nothing about whether the order is finished.

#### Can this account trade?

[`accounts::Account::trading_status`] is one request and answers before the
venue answers with a rejection.

```rust
let status = account.trading_status().await?;

if status.is_blocked() {
    println!("this account cannot trade");
} else if !status.is_known_blocked() {
    // The venue did not send the flags, so "not blocked" is not something
    // this process actually knows. Every flag is `Option<bool>`, and a flag
    // the broker omitted is unknown rather than false.
    println!("unverified: the venue reported neither flag");
}

println!("day trades used: {:?}", status.day_trade_count);
```

#### The equity curve

[`accounts::Account::net_liq_history`] is open/high/low/close of net
liquidating value over time. `time-back` and an explicit window are one
enum, so a request carrying both cannot be built.

```rust
let bars = account
    .net_liq_history(&NetLiqHistoryFilter::back(TimeBack::OneMonth))
    .await?;

let peak = bars.iter().filter_map(|bar| bar.high).max();
```

**Live only**: the venue's sandbox page lists this endpoint as unavailable
in certification. It is also served by a different system, which spells its
JSON in camelCase and its timestamps as JVM `ZonedDateTime` — so
[`prelude::NetLiqOhlc::time`] is a `String` rather than a `DateTime`, and
every field accepts both spellings.

#### Margin and risk

What an order will consume, and what the account may hold.
[`accounts::Account::margin_requirements`] is the standing requirement,
nested total → underlying → strategy, because the per-strategy figures are
what explain the total.

```rust
let report = account.margin_requirements().await?;
for group in &report.groups {
    println!("{:?}: {:?}", group.underlying_symbol, group.margin_requirement);
}

let limit = account.position_limit().await?;
println!("largest equity order: {:?}", limit.equity_order_size);
```

[`accounts::Account::estimate_margin`] is **not** the order preflight.
[`accounts::Account::dry_run`] asks whether the venue would accept an order;
this asks how much buying power it would take. Neither routes anything, and
there is no path from this one to a placement — it takes
[`prelude::MarginOrderRequest`], which carries the account number and
underlying symbol an [`prelude::Order`] does not, so an order cannot be
handed to it by accident.

One to [`prelude::MAX_MARGIN_LEGS`] unique legs, checked locally: a repeated
leg is almost always one leg written twice, and a doubled requirement is the
kind of wrong that looks plausible.

#### Transactions

The ledger: fills, fees, dividends, assignments, cash movements. It is the
only place a P&L can be reconciled from — an order says what was asked for,
a transaction says what happened and what it cost.

```rust
let page = account
    .transactions(
        &TransactionFilter::new()
            .with_types(TransactionTypes::Several(vec![
                TransactionType::Trade,
                TransactionType::ReceiveDeliver,
            ]))
            .with_page(PageRequest::first().with_per_page(250)),
    )
    .await?;

for row in &page {
    // `None` means the venue sent nothing, never zero. A commission that
    // defaults to zero is a P&L that is quietly wrong.
    println!("{:?} {:?}", row.transaction_sub_type, row.net_value);
}
```

The venue documents `type` and `types` as mutually exclusive, so they are
one enum here and a request carrying both cannot be built.

[`accounts::Account::total_fees`] takes an `Option<NaiveDate>`; passing
`None` omits the parameter and leaves the venue's own "today" in place,
rather than substituting this machine's idea of the date.

### Instrument listings

The instrument listings paginate, and each takes a typed filter rather than
a row of positional `Option`s. A filter that sets nothing sends nothing, so
the venue's own defaults are what answer.

```rust
use tastytrade::prelude::*;

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
```

Array parameters are repeated keys, which is how the venue spells them —
`product-code[]=ES&product-code[]=6A`, not one comma-joined value:

```rust
let page = tasty
    .list_futures(&FutureFilter::for_product_codes(&["ES", "6A"]))
    .await?;
println!("{} contract(s) of {}", page.len(), page.pagination.total_items);
```

A value the venue adds later still round-trips: [`prelude::Lendability`]
and the other wire enums keep an unrecognised value verbatim rather than
failing, so a new classification never makes an instrument disappear.

#### Finding an instrument

Two searches, with different encodings. `search_symbols` is a prefix search
whose term is a **path segment**; `search_instruments` spans every
instrument type and takes classification filters that are **comma-joined
into one parameter each**, which is the opposite of the listings above.

```rust
// A class separator in the term is encoded, so this is a search for
// `BRK/B` rather than a search of `/symbols/search/BRK` for `B`.
for hit in tasty.search_symbols("BRK/B").await? {
    println!("{} — {:?}", hit.symbol, hit.description);
}

let results = tasty
    .search_instruments(
        &InstrumentSearchFilter::for_query("gold")
            .with_types(&["Equity", "Future"])
            .with_limit(10),
    )
    .await?;
```

`limit` is capped at [`prelude::MAX_SEARCH_RESULTS`] and an over-large one
fails locally as a non-retryable precondition, before anything is sent.

`ai_search_token()` mints a short-lived third-party credential for AI
search. It is treated like every other secret here — never in `Debug`,
`Display`, a log or an error — and it is handed back rather than used,
because the service it authenticates is not part of this API.

### Real-time Data

Market data comes over DXLink. All eleven event types the feed models are
routed: quotes, regular and extended-hours trade prints, Greeks, candles,
summaries, time and sale, profiles, underlyings, theoretical prices and
series. A subscription names the ones it wants and the channel is
configured for exactly those.

```rust
// Create a quote streamer
use tastytrade::{Symbol, TastyTrade};
use tastytrade::utils::config::TastyTradeConfig;
use tastytrade::dxfeed::{self, EventKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TastyTradeConfig::from_env();
    let tasty = TastyTrade::connect(&config).await?;
    let mut quote_streamer = tasty.create_quote_streamer().await?;
    let mut quote_sub = quote_streamer.create_sub([EventKind::Quote, EventKind::Greeks]).await?;

    // Add symbols to subscribe to
    quote_sub.add_symbols(&[Symbol("AAPL".to_string())]).await?;

    // Listen for events
    if let Ok(dxfeed::Event { sym, data }) = quote_sub.get_event().await {
        match data {
            dxfeed::EventData::Quote(quote) => {
                println!("Quote for {}: {}/{}", sym, quote.bid_price, quote.ask_price);
            }
            _ => {}
        }
    }
    Ok(())
}
```

#### Candles

Candles are the only route to a price series in this crate — there is no
REST endpoint for one — and the only subscription that needs more than a
symbol. A candle is addressed by a symbol that carries its period,
`AAPL{=5m}`, so two periods of one underlying are two different streamer
symbols and never deliver into each other.

```rust
use chrono::{Duration, Utc};
use tastytrade::{Symbol, TastyTrade};
use tastytrade::dxfeed::{CandlePeriod, EventData, EventKind};

let mut streamer = tasty.create_quote_streamer().await?;
let mut bars = streamer.create_sub([EventKind::Candle]).await?;

// `from_time` is required, not optional: a candle subscription without one
// replays an unbounded history. A day of one-minute bars is about 1440
// events per symbol.
bars.add_candles(
    &[Symbol("AAPL".to_string())],
    CandlePeriod::minutes(5)?,
    Utc::now() - Duration::days(2),
)
.await?;

if let Ok(event) = bars.get_event().await
    && let EventData::Candle(candle) = event.data
{
    // `event.sym` is `AAPL{=5m}`, period included.
    println!("{}: o {} c {}", event.sym, candle.open, candle.close);
}
```

A reconnect resumes each candle series one millisecond past the last bar
delivered **contiguously**, rather than replaying the original start —
otherwise every reconnect would re-send a history the consumer already has.

#### Falling behind

A subscription's buffer is bounded, so a consumer that does not keep up
loses events rather than stalling every other subscription. That is the
right trade for quotes and the wrong one for a price series, so it is
observable:

```rust
if bars.lagged() > 0 {
    // Those events are gone; reading faster will not bring them back.
    println!("{} events lost", bars.lagged());
}
```

For candles it is also recoverable across a reconnect: a dropped bar stops
the resume point advancing, so the next connection asks for it again. Within
one connection a dropped bar stays dropped. Size the buffer with
[`QuoteStreamer::create_sub_with_capacity`](streaming::quote_streamer::QuoteStreamer::create_sub_with_capacity)
when the default does not fit the history you are asking for.

### Account streaming

The account websocket publishes a **full object** on every change — never a
diff — for orders, balances, positions, quote alerts and tastytrade's
public watchlists. The fills inside an order's `legs` are the only place an
executed price reaches this crate: no REST endpoint returns one.

```rust
use tastytrade::prelude::*;

let streamer = tasty.create_account_streamer().await?;
for account in &tasty.accounts().await? {
    streamer.subscribe_to_account(account).await?;
}

match streamer.get_event().await? {
    AccountEvent::Notification(notification) => match notification.payload {
        NotificationPayload::Order(order) => {
            for leg in &order.legs {
                for fill in &leg.fills {
                    println!("{:?} at {:?}", fill.quantity, fill.fill_price);
                }
            }
        }
        // A notification type this crate does not model yet still arrives,
        // with its payload. Nothing is discarded.
        NotificationPayload::Unsupported(payload) => {
            println!("{} arrived untyped ({} bytes)", notification.kind, payload.len());
        }
        _ => {}
    },
    AccountEvent::Unknown(unknown) => println!("unplaceable: {:?}", unknown.kind),
    _ => {}
}
```

Anything that is JSON reaches the caller. A `type` nobody here recognises,
a payload that does not match its model, and a frame that is neither a
notification nor an acknowledgement all arrive with the payload intact —
`RawPayload` renders as a byte count, so reading it takes
[`RawPayload::expose`](prelude::RawPayload::expose) and is one grep away
from an audit. Only bytes that are not JSON are dropped, and that is
reported without the frame or the serde error.

### Environments

`TastyTradeConfig::from_env` selects the **certification** environment by
default (`api.cert.tastyworks.com`). Production is a deliberate opt-in:

```shell
TASTYTRADE_USE_DEMO=false   # production — orders placed here are real
```

Only a value that parses as `false` selects production. A missing, empty or
misspelled variable resolves to certification, so a typo cannot be what
points an order at a funded account.

Connecting without `TASTYTRADE_CLIENT_SECRET` and
`TASTYTRADE_REFRESH_TOKEN` fails locally with
`TastyTradeError::ConfigError` and never reaches the network.

A session is bound to the deployment it authenticated against: it will not
present a certification token to production, and it will not send the
client secret to a host it did not authenticate with.

### Authorizing other people's accounts

A **trusted third-party** application — one tastytrade has reviewed — sends
a customer to the authorization page and exchanges the code it gets back:

```rust
use tastytrade::TastyTrade;
use tastytrade::oauth::{AuthorizationRequest, Scope};
use tastytrade::utils::config::TastyTradeConfig;

let config = TastyTradeConfig::from_env();

let request = AuthorizationRequest::new(&config.client_id, &config.redirect_uri)
    .with_scopes([Scope::Read, Scope::Trade])
    // Tie this to the browser session that started the flow. This crate
    // does not invent one: a nonce the application cannot correlate
    // proves nothing.
    .with_state(state);

// Send the customer here. The URL carries no secret.
let url = request.authorize_url(config.environment())?;

// …they come back to your redirect URI with `code` and `state`.
request.verify_state(returned_state)?;
let tasty = TastyTrade::connect_with_authorization_code(&config, code).await?;

// Store this. It does not expire, and having it means never sending the
// customer through the authorization page again.
let refresh_token = tasty.refresh_token().await;
```

### Placing an order

Placement goes through a review the venue's warnings cannot be skipped
past silently:

```rust
let receipt = account.review_order(order).await?;
println!("buying power effect: {}", receipt.result().buying_power_effect.change_in_buying_power);

if !receipt.is_clean() {
    for warning in receipt.warnings() {
        println!("warning: {}", warning.message);
    }
    // accept_with_warnings is for a person who has read the above and
    // still wants the order. Reaching for it automatically defeats it.
    return Ok(());
}
let reviewed = receipt.accept()?;

// Certification only. An example that places on production is an example
// somebody runs on production.
if config.use_demo {
    account.place_reviewed_order(reviewed).await?;
}
```

`Account::place_order` still exists for callers that manage the review
themselves, but it carries no evidence that a review happened.

### Logging

This crate emits `tracing` events and does **not** install a subscriber on
your behalf: process-global logging belongs to the application. Loading
configuration touches no global state, so a program that already owns
`tracing` keeps its own setup.

Binaries that do not want to build one can opt in:

```rust
use tastytrade::utils::logger::{try_setup_logger, LoggerInit};

match try_setup_logger() {
    LoggerInit::Installed => {}
    LoggerInit::AlreadyInstalled => { /* the application owns it */ }
    LoggerInit::Unsupported => { /* wasm32 */ }
}
```

Without a subscriber the environment warnings above are not printed, so a
binary that cares about them should install one before building the config.

 ## Setup Instructions

 1. Clone the repository:
 ```shell
 git clone https://github.com/joaquinbejar/tastytrade
 cd tastytrade
 ```

 2. Build the project:
 ```shell
 make build
 ```

 3. Run tests:
 ```shell
 make test
 ```

 4. Format the code:
 ```shell
 make fmt
 ```

 5. Run linting:
 ```shell
 make lint
 ```

 6. Clean the project:
 ```shell
 make clean
 ```

 7. Run the project:
 ```shell
 make run
 ```

 8. Fix issues:
 ```shell
 make fix
 ```

 9. Run pre-push checks:
 ```shell
 make pre-push
 ```

 10. Generate documentation:
 ```shell
 make doc
 ```

 11. Publish the package:
 ```shell
 make publish
 ```

 12. Generate coverage report:
 ```shell
 make coverage
 ```


### CLI Example

This crate also includes a sample CLI application in the `tastytrade-cli` directory
that demonstrates a portfolio viewer with real-time updates.

It reads its credentials from the environment, and takes `--config` to read
them from a JSON file instead. Neither credential is a flag on purpose: a
secret given on the command line is visible to every process on the machine
and is kept in the shell history.

 ```shell
 export TASTYTRADE_CLIENT_SECRET=...
 export TASTYTRADE_REFRESH_TOKEN=...
 export TASTYTRADE_USE_DEMO=true      # certification, the safe default
 cargo run -p tastytrade-cli
 ```

### Migrating from 0.3

Every authentication surface changed, because the API behind it was
retired. This is a breaking release and `cargo semver-checks` reports it as
one.

| Removed | Replacement |
|---|---|
| `TastyTrade::login(&config)` | [`TastyTrade::connect`] |
| `TastyTrade::default()` | [`TastyTrade::from_env`] |
| `LoginCredentials`, `LoginResponse`, `LoginResponseUser` | [`oauth::TokenResponse`] |
| `TastyTradeConfig::username`, `::password` | `client_secret`, `refresh_token` |
| `TastyTradeConfig::remember_me`, `TASTYTRADE_REMEMBER_ME` | nothing — it configured a retired API |
| `TASTYTRADE_USERNAME`, `TASTYTRADE_PASSWORD` | `TASTYTRADE_CLIENT_SECRET`, `TASTYTRADE_REFRESH_TOKEN` |
| CLI `--login` | CLI `--config` |

There is no deprecation window: a deprecated `login()` would still be a
call to an endpoint that no longer exists, so leaving one in place would
only move the failure from compile time to run time.

 ## Testing

 To run unit tests:
 ```shell
 make test
 ```

 To run tests with coverage:
 ```shell
 make coverage
 ```

 ## Contribution and Contact

 We welcome contributions to this project! If you would like to contribute, please follow these steps:

 1. Fork the repository.
 2. Create a new branch for your feature or bug fix.
 3. Make your changes and ensure that the project still builds and all tests pass.
 4. Commit your changes and push your branch to your forked repository.
 5. Submit a pull request to the main repository.

 If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

 **Joaquín Béjar García**
 - Email: jb@taunais.com
 - GitHub: [joaquinbejar](https://github.com/joaquinbejar)

 We appreciate your interest and look forward to your contributions!




## Contribution and Contact

We welcome contributions to this project! If you would like to contribute, please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

**Joaquín Béjar García**
- Email: jb@taunais.com
- GitHub: [joaquinbejar](https://github.com/joaquinbejar)

We appreciate your interest and look forward to your contributions!

## ✍️ License

Licensed under MIT license
