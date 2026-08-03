# Account streamer frames

Reference frames for the account websocket, one per notification the four
documented actions produce.

**None of these is a captured venue frame.** They come from two places, and
each is labelled:

* **documented** — copied from tastytrade's Streaming Account Data guide,
  with the account number replaced by a sentinel;
* **derived** — assembled from the published swagger definition for the same
  object, because the guide does not show that notification.

Capturing a real frame for each of these needs a certification session with
activity on it, which is `/smoke`'s job. Until that happens, a derived frame is
evidence about the *shape* the venue publishes and not about which fields it
actually sends — which is why every field in the corresponding Rust type is
`Option` unless the venue's own schema marks it required.

The envelope is always the same:

```json
{ "type": "<Type>", "data": { }, "timestamp": 1688595114405 }
```

`timestamp` is epoch milliseconds. Streamer messages always contain a full
object, never a diff.

---

## `connect` — Order (documented)

The guide's own worked example. The `fills` inside `legs` are the only place an
executed price reaches this crate: no REST endpoint returns one.

An order leg is filled many times in practice — the guide's example is a
hundred one-share fills for a hundred-share order — and each is published as it
is processed, so a `Filled` order can arrive with one leg's fills and be
followed immediately by the same order with all of them.

```json
{
  "type": "Order",
  "data": {
    "id": 1,
    "account-number": "5WT00000",
    "time-in-force": "Day",
    "order-type": "Market",
    "size": 100,
    "underlying-symbol": "AAPL",
    "underlying-instrument-type": "Equity",
    "status": "Filled",
    "cancellable": false,
    "editable": false,
    "edited": false,
    "ext-exchange-order-number": "12345",
    "ext-client-order-id": "67890",
    "ext-global-order-number": 1111,
    "received-at": "2023-07-05T19:07:32.444+00:00",
    "updated-at": 1688584052750,
    "in-flight-at": "2023-07-05T19:07:32.494+00:00",
    "live-at": "2023-07-05T19:07:32.495+00:00",
    "destination-venue": "TEST_A",
    "user-id": 99,
    "username": "coolperson",
    "terminal-at": "2023-07-05T19:07:32.737+00:00",
    "legs": [
      {
        "instrument-type": "Equity",
        "symbol": "AAPL",
        "quantity": 100,
        "remaining-quantity": 0,
        "action": "Buy to Open",
        "fills": [
          {
            "ext-group-fill-id": "0",
            "ext-exec-id": "1122",
            "fill-id": "24_TW::TEST_A47504::20230705.1179-TEST_FILL",
            "quantity": 100,
            "fill-price": "100.0",
            "filled-at": "2023-07-05T19:07:32.496+00:00",
            "destination-venue": "TEST_A"
          }
        ]
      }
    ]
  },
  "timestamp": 1688595114405
}
```

Two shapes worth noting, because they are why the corresponding Rust fields
look odd:

* `updated-at` is an **integer** here and a string in the swagger definition.
  Both sources are official and they disagree, so `LiveOrderRecord::updated_at`
  is a tolerant `Option<String>` that keeps whatever arrived. Every other
  timestamp on the order is `date-time` and is typed.
* `user-id` is an integer here and a string in the swagger. It is typed as
  `Option<String>`, tolerant of both, for the same reason.

## `connect` — AccountBalance (derived)

Derived from `AccountBalance` in `account-positions-api-swagger_20240501`.
Which fields the venue sends depends on what the account is permitted to trade:
a cash account has no futures margin figures.

```json
{
  "type": "AccountBalance",
  "data": {
    "account-number": "5WT00000",
    "currency": "USD",
    "cash-balance": "10000.0",
    "net-liquidating-value": "12345.67",
    "maintenance-requirement": "0.0",
    "equity-buying-power": "20000.0",
    "derivative-buying-power": "10000.0",
    "day-trading-buying-power": "40000.0",
    "updated-at": "2026-08-03T14:30:00.000+00:00"
  },
  "timestamp": 1688595114405
}
```

## `connect` — CurrentPosition (derived)

Derived from `CurrentPosition` in the same document. Which fields appear
depends on the instrument: an equity has no `expires-at`, a future has no
`deliverable-type`.

```json
{
  "type": "CurrentPosition",
  "data": {
    "account-number": "5WT00000",
    "symbol": "AAPL",
    "instrument-type": "Equity",
    "underlying-symbol": "AAPL",
    "quantity": "100",
    "quantity-direction": "Long",
    "close-price": "100.0",
    "average-open-price": "99.5",
    "multiplier": 1,
    "cost-effect": "Debit",
    "is-suppressed": false,
    "is-frozen": false,
    "restricted-quantity": "0",
    "realized-day-gain": "0.0",
    "realized-today": "0.0",
    "created-at": "2026-08-01T14:30:00.000+00:00",
    "updated-at": "2026-08-03T14:30:00.000+00:00"
  },
  "timestamp": 1688595114405
}
```

## `quote-alerts-subscribe` — QuoteAlert (derived)

Derived from `QuoteAlertDeserializer` in `quote-alerts-api-swagger`. Alerts are
per **user**, not per account, so nothing here carries an account number.

```json
{
  "type": "QuoteAlert",
  "data": {
    "alert-external-id": "alert-1",
    "user-external-id": "U0001",
    "symbol": "AAPL",
    "dx-symbol": "AAPL",
    "instrument-type": "Equity",
    "field": "Last",
    "operator": ">",
    "threshold": "200.00",
    "threshold-numeric": 200.0,
    "provider": "dxfeed",
    "created-at": "2026-08-01T12:00:00.000+00:00",
    "triggered-at": "2026-08-03T14:30:00.000-04:00",
    "completed-at": "2026-08-03T14:30:00.100-04:00",
    "expires-at": "1788595114405"
  },
  "timestamp": 1688595114405
}
```

`expires-at` is the field the sources disagree about: the swagger types it as a
string with no format, and the tastyware Python SDK — built against real frames
— types it as an integer. `QuoteAlert::expires_at` accepts both and keeps the
text.

## `public-watchlists-subscribe` — PublicWatchlists (derived)

Derived from `Watchlist` and the entry schema in `watchlists-api-swagger`.

```json
{
  "type": "PublicWatchlists",
  "data": {
    "name": "High Options Volume",
    "group-name": "tastytrade",
    "order-index": 3,
    "cms-id": "blt-high-options-volume",
    "watchlist-entries": [
      { "symbol": "AAPL", "instrument-type": "Equity" },
      { "symbol": "/ES", "instrument-type": "Future" }
    ]
  },
  "timestamp": 1688595114405
}
```

---

## Acknowledgements and refusals (documented)

These have no `type` and are not notifications. `connect` echoes the accounts
it subscribed:

```json
{
  "status": "ok",
  "action": "connect",
  "web-socket-session-id": "5b6e2799",
  "value": ["5WT00000", "5WT00001"],
  "request-id": 2
}
```

```json
{
  "status": "ok",
  "action": "heartbeat",
  "web-socket-session-id": "5b6e2799",
  "request-id": 1
}
```

`request-id` comes back **only if the request carried one**, and this crate
sends none. A required `request-id` in `StatusMessage` is why every
acknowledgement used to fail the untagged decode and disappear.

A refusal carries `status: "error"` and a message:

```json
{
  "status": "error",
  "action": "quote-alerts-subscribe",
  "web-socket-session-id": "5b6e2799",
  "message": "connect-not-completed"
}
```

`connect-not-completed` is what the venue answers when any other subscription
arrives before a `connect` has landed. `message` is venue prose: it can name an
account or a subscription, so it belongs in front of a person and never in a
log line.

## Frames with no home

Anything else reaches the caller as `AccountEvent::Unknown` with the bytes
intact — a notification type the venue adds tomorrow, or a frame that is
neither typed nor an acknowledgement. Only bytes that are not JSON are dropped,
and that is reported at WARN with a classification, a byte count and a
position, never with the serde error or the frame.
