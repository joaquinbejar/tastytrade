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
    /// Stands in for an OAuth2 access token.
    pub const ACCESS_TOKEN: &str = "SENTINEL-access-token-6Xq2";
    /// Stands in for an OAuth2 refresh token.
    pub const REFRESH_TOKEN: &str = "SENTINEL-refresh-token-9Zk4";
    /// Stands in for the OAuth application's client secret.
    pub const CLIENT_SECRET: &str = "SENTINEL-client-secret-4Tn8";
    /// Stands in for an OpenID Connect identity token.
    pub const ID_TOKEN: &str = "SENTINEL-id-token-7Jp3";
    /// Stands in for an account number.
    pub const ACCOUNT_NUMBER: &str = "SENTINEL-5WX00042";
    /// Stands in for an account nickname.
    pub const NICKNAME: &str = "SENTINEL-nickname-Retirement";
    /// Stands in for a cash balance.
    pub const BALANCE: &str = "SENTINEL-1234567.89";
}

/// A successful `POST /oauth/token` body carrying every sentinel token.
///
/// Not wrapped in the tastytrade `data` envelope: the token endpoint answers
/// with the plain OAuth2 object from RFC 6749 §5.1, which is why it needs a
/// decode path of its own.
pub fn token_response_body() -> String {
    format!(
        r#"{{
            "access_token": "{}",
            "refresh_token": "{}",
            "token_type": "Bearer",
            "expires_in": 900,
            "id_token": "{}"
        }}"#,
        sentinel::ACCESS_TOKEN,
        sentinel::REFRESH_TOKEN,
        sentinel::ID_TOKEN
    )
}

/// The same, with a lifetime that is already inside the refresh margin.
///
/// A client built from this refreshes before its very next request, which is
/// how expiry is exercised without waiting fifteen minutes.
pub fn expiring_token_response_body() -> String {
    format!(
        r#"{{
            "access_token": "{}",
            "token_type": "Bearer",
            "expires_in": 0
        }}"#,
        sentinel::ACCESS_TOKEN
    )
}

/// An RFC 6749 §5.2 refusal.
pub fn token_error_body(code: &str) -> String {
    format!(
        r#"{{
            "error": "{code}",
            "error_description": "the venue rejected {} for this application"
        }}"#,
        sentinel::REFRESH_TOKEN
    )
}

/// A `GET /customers/me/accounts` body with one account that decodes and one
/// that cannot, shaped like real accounts so the sentinels are exactly what a
/// leak would expose.
///
/// The healthy account matters: a listing where *nothing* decodes is an error
/// rather than a skip, so without it this exercises a different path.
pub fn partially_unparseable_accounts_body() -> String {
    format!(
        r#"{{
            "data": {{
                "items": [
                    {{
                        "account": {{
                            "account-number": "5WX00001",
                            "nickname": "Healthy",
                            "account-type-name": "Individual",
                            "margin-or-cash": "Margin",
                            "opened-at": "2025-01-14T10:22:41.000+00:00"
                        }},
                        "authority-level": "owner"
                    }},
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

/// A listing where every item fails, which is a broken model rather than a
/// thin response.
pub fn wholly_unparseable_accounts_body() -> String {
    format!(
        r#"{{
            "data": {{
                "items": [
                    {{
                        "account": {{
                            "account-number": "{}",
                            "nickname": "{}",
                            "margin-or-cash": {}
                        }},
                        "authority-level": "owner"
                    }}
                ]
            }},
            "context": "/customers/me/accounts"
        }}"#,
        sentinel::ACCOUNT_NUMBER,
        sentinel::BALANCE,
        12345
    )
}
