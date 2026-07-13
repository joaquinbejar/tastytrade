use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Placeholder used instead of secret values in Debug/Display output.
const REDACTED: &str = "***";

/// Login credentials for authentication.
///
/// This struct holds the login information required for authentication, including
/// the username, password, and a "remember me" flag.  It's designed for
/// serialization with kebab-case renaming for compatibility with external APIs.
///
/// `Debug` and `Display` are implemented manually so the password is never
/// written to logs.
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoginCredentials {
    /// The username for login.
    pub login: String,
    /// The password for login.
    pub password: String,
    /// A flag indicating whether to remember the login.
    pub remember_me: bool,
}

impl fmt::Debug for LoginCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginCredentials")
            .field("login", &self.login)
            .field("password", &REDACTED)
            .field("remember_me", &self.remember_me)
            .finish()
    }
}

impl fmt::Display for LoginCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[allow(dead_code)]
#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Represents a user in a login response.  This struct is used for deserializing the JSON response
/// received after a successful login.  The `#[serde(rename_all = "kebab-case")]` attribute ensures
/// that the fields in the JSON response are matched to the struct fields correctly, even if the
/// casing is different (e.g., "external-id" in JSON will map to `external_id` in the struct).
pub struct LoginResponseUser {
    /// The user's email address.
    pub email: String,
    /// The user's username.
    pub username: String,
    /// The user's external ID.
    pub external_id: String,
}

/// Represents the response received after a successful login.
///
/// This struct is used for deserializing the JSON response.
/// The `#[serde(rename_all = "kebab-case")]` attribute ensures that the
/// fields in the JSON response are matched to the struct fields correctly,
/// even if the casing is different (e.g., "session-token" in JSON will map to
/// `session_token` in the struct).
///
/// `Debug` and `Display` are implemented manually so `session_token` and
/// `remember_token` are never written to logs.
#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoginResponse {
    /// The user information associated with the login.
    pub user: LoginResponseUser,
    /// The session token.
    pub session_token: String,
    /// The remember token (optional).
    pub remember_token: Option<String>,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginResponse")
            .field("user", &self.user)
            .field("session_token", &REDACTED)
            .field(
                "remember_token",
                &self.remember_token.as_deref().map(|_| REDACTED),
            )
            .finish()
    }
}

impl fmt::Display for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_response() -> LoginResponse {
        LoginResponse {
            user: LoginResponseUser {
                email: "user@example.com".to_string(),
                username: "user".to_string(),
                external_id: "ext-1".to_string(),
            },
            session_token: "super-secret-session-token".to_string(),
            remember_token: Some("super-secret-remember-token".to_string()),
        }
    }

    #[test]
    fn test_login_response_debug_redacts_tokens() {
        let resp = sample_response();
        for output in [format!("{resp:?}"), format!("{resp}")] {
            assert!(!output.contains("super-secret-session-token"));
            assert!(!output.contains("super-secret-remember-token"));
            assert!(output.contains("***"));
            assert!(output.contains("user@example.com"));
        }
    }

    #[test]
    fn test_login_credentials_debug_redacts_password() {
        let creds = LoginCredentials {
            login: "user".to_string(),
            password: "super-secret-password".to_string(),
            remember_me: true,
        };
        for output in [format!("{creds:?}"), format!("{creds}")] {
            assert!(!output.contains("super-secret-password"));
            assert!(output.contains("***"));
        }
    }
}
