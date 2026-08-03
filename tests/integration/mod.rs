//! Network-free integration suite.
//!
//! Declared by `Cargo.toml` as the `tests` target. Everything runs against a
//! loopback venue this process owns: no brokerage credentials, no live API, no
//! wall-clock dependence. Anything that genuinely needs the real venue is an
//! example or part of the `/smoke` battery, not a test.
//!
//! Secrets in fixtures are sentinels — distinctive strings that cannot occur
//! by accident — so a test can assert a credential did *not* reach a log or an
//! error, rather than merely asserting the happy path still works.

mod support;

mod account_stream;
mod paths;
mod rest;
