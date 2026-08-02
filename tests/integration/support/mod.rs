//! Test support for the network-free integration suite.
//!
//! Everything here talks to a loopback socket that this process owns. No
//! brokerage credentials, no live venue, no wall-clock dependence, so the
//! suite runs in CI and on a laptop with the network unplugged.

pub mod capture;
pub mod venue;

pub use capture::{CapturedLogs, capture_logs_at};
pub use venue::{MockVenue, Route};

/// Values that must never appear in a log line, an error, or any other place
/// the crate hands back to a caller. Every sentinel is distinctive enough that
/// a substring search cannot produce a false positive.
pub mod sentinel {
    /// Stands in for a session token.
    pub const SESSION_TOKEN: &str = "SENTINEL-session-token-6Xq2";
    /// Stands in for a remember token.
    pub const REMEMBER_TOKEN: &str = "SENTINEL-remember-token-9Zk4";
    /// Stands in for the account password.
    pub const PASSWORD: &str = "SENTINEL-password-4Tn8";
    /// Stands in for an account number.
    pub const ACCOUNT_NUMBER: &str = "SENTINEL-5WX00042";
    /// Stands in for an account nickname.
    pub const NICKNAME: &str = "SENTINEL-nickname-Retirement";
    /// Stands in for a cash balance.
    pub const BALANCE: &str = "SENTINEL-1234567.89";
}

/// A successful `POST /sessions` body carrying both sentinel tokens.
pub fn login_response_body() -> String {
    format!(
        r#"{{
            "data": {{
                "user": {{
                    "email": "someone@example.com",
                    "username": "someone",
                    "external-id": "U0001"
                }},
                "session-token": "{}",
                "remember-token": "{}"
            }},
            "context": "/sessions"
        }}"#,
        sentinel::SESSION_TOKEN,
        sentinel::REMEMBER_TOKEN
    )
}

/// A `GET /customers/me/accounts` body whose single item cannot deserialize,
/// shaped like a real account so the sentinels are exactly what a leak would
/// expose.
pub fn unparseable_accounts_body() -> String {
    format!(
        r#"{{
            "data": {{
                "items": [
                    {{
                        "account": {{
                            "account-number": "{}",
                            "nickname": "{}",
                            "cash-balance": "{}",
                            "margin-or-cash": {}
                        }},
                        "authority-level": "owner"
                    }}
                ]
            }},
            "context": "/customers/me/accounts"
        }}"#,
        sentinel::ACCOUNT_NUMBER,
        sentinel::NICKNAME,
        sentinel::BALANCE,
        // A number where the model wants a string: the serde error renders the
        // rejected value, which is the leak path this suite guards.
        12345
    )
}
