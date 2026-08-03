# Account streamer frame fixtures

The frames the account-streaming tests decode. One file per notification, read
by `include_str!` so a deleted or renamed fixture fails the build rather than
silently reducing coverage.

## The suffix is the provenance, and it is the point

| Suffix | What it means |
|---|---|
| `.documented` | Copied from tastytrade's Streaming Account Data guide, with identifiers replaced by sentinels. Evidence about what the venue actually sends. |
| `.derived` | Assembled from the published swagger for the same object. Evidence about **shape only** — not about which fields the venue sends, nor about which of two contradictory types a field really has. |
| `.captured` | Recorded from a live certification session and redacted. The real thing. |

**There are no `.captured` fixtures yet.** Capturing them needs an OAuth
application and grant against certification, which is
[#96](https://github.com/joaquinbejar/tastytrade/issues/96).

A derived fixture fed to a type derived from the same swagger mostly proves the
two agree with each other. It catches a rename, a wrong `serde` attribute, a
field that moved — and it cannot catch a field the venue names differently from
its own documentation, or a shape the documentation gets wrong. Two are known
to be wrong somewhere already: `updated-at` and `user-id` on an order are
integers in the guide's worked example and strings in the swagger.

## Capturing the real ones

```shell
cargo run -p accounts-status --bin capture_frames
```

Connects to certification, listens for a bounded time, and writes one
`<type>.captured.json` per notification type it sees — redacting account
numbers, usernames and user identifiers on the way out. Then:

1. read every file it wrote, and check the redaction did its job;
2. delete the `.derived` fixture it supersedes;
3. point the test at the `.captured` one;
4. reconcile anything that disagrees with the type, which is the whole reason
   for doing this.

Nothing is committed automatically. A fixture is evidence, and evidence gets
looked at before it is checked in.
