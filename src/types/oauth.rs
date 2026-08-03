//! OAuth2 credentials, grants and token responses.
//!
//! tastytrade decommissioned `POST /sessions` on 2026-02-11; OAuth2 is the
//! only authentication the venue still accepts. Two flows are documented and
//! both live here:
//!
//! * the **personal refresh-token grant**, where you hold a client secret and
//!   a long-lived refresh token you created on `my.tastytrade.com`, and
//! * the **authorization-code grant**, for a trusted third-party application
//!   that sends a customer to tastytrade's authorization page and exchanges
//!   the code it gets back.
//!
//! Everything secret in this module is a newtype whose `Debug` and `Display`
//! print `***`. Reading the real value takes a call to `expose_secret`, which
//! is deliberately ugly: it is the one line a reviewer has to look at to know
//! where a credential could escape.

use std::fmt;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::TastyTradeError;
use crate::api::base::TastyResult;
use crate::error::Environment;

/// Written in place of any secret value.
const REDACTED: &str = "***";

/// Where a customer authorizes a production application.
const AUTHORIZE_URL: &str = "https://my.tastytrade.com/auth.html";

/// The certification equivalent.
const AUTHORIZE_DEMO_URL: &str = "https://cert-my.staging-tasty.works/auth.html";

/// How long before expiry an access token is treated as already expired.
///
/// A token that has sixty seconds left is a token that will expire in the
/// middle of the next request, and a 401 halfway through placing an order is
/// the one failure this crate cannot retry its way out of.
pub(crate) const REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// What the venue answers with when it does not say.
///
/// The documented lifetime is fifteen minutes. Used only when `expires_in` is
/// missing from the response, which the OAuth2 spec permits.
const DEFAULT_TOKEN_LIFETIME: Duration = Duration::from_secs(15 * 60);

/// The longest lifetime this crate will believe.
///
/// `expires_in` is a number chosen by the endpoint, and `Instant + Duration`
/// **panics** when the sum leaves the platform's range — so an absurd value in
/// an otherwise successful response could abort the caller's process inside
/// `connect`. A library does not get to do that.
///
/// Twenty-four hours is far past the documented fifteen minutes and still
/// bounded. Clamping errs towards refreshing sooner than the venue said, which
/// costs one request; believing an unbounded lifetime costs a token that is
/// never renewed.
const MAX_TOKEN_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

/// Defines a secret string whose `Debug` and `Display` never reveal it.
///
/// Deliberately not `Serialize`: a secret that can be serialized is a secret
/// that ends up in a configuration file, a structured log or a `DebugPretty`
/// rendering by accident. Reading it takes `expose_secret`.
macro_rules! secret_string {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Clone, Default, PartialEq, Eq, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps `value`.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// The real value.
            ///
            /// Every call is a place a credential can leave the process. There
            /// are a handful in this crate — the token request body and the
            /// `Authorization` header — and there should not be more.
            pub fn expose_secret(&self) -> &str {
                &self.0
            }

            /// Whether this carries nothing usable.
            ///
            /// Whitespace does not count: `TASTYTRADE_CLIENT_SECRET=" "` is a
            /// shell accident, not a credential, and sending it to the venue
            /// only turns a local mistake into a remote refusal.
            pub fn is_blank(&self) -> bool {
                self.0.trim().is_empty()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({REDACTED})", stringify!($name))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(REDACTED)
            }
        }
    };
}

secret_string! {
    /// The secret shown once when an OAuth application is created.
    ///
    /// Presented on every call to the token endpoint, in both grants.
    ClientSecret
}

secret_string! {
    /// A long-lived credential that mints access tokens.
    ///
    /// tastytrade's refresh tokens do not expire. Losing one means deleting
    /// the grant on `my.tastytrade.com`, so it is worth at least as much care
    /// as a password.
    RefreshToken
}

secret_string! {
    /// A short-lived bearer credential, good for about fifteen minutes.
    ///
    /// Sent as `Authorization: Bearer <token>` on every REST request, and in
    /// the account websocket's `auth-token` field with the same prefix.
    AccessToken
}

secret_string! {
    /// The single-use code an authorization redirect carries back.
    AuthorizationCode
}

secret_string! {
    /// The OpenID Connect identity token, returned only for the `openid` scope.
    ///
    /// This crate does not verify or decode it. It is a signed assertion about
    /// the customer, so it is handled as a secret and handed straight back.
    IdToken
}

impl AccessToken {
    /// The value of an `Authorization` header carrying this token.
    ///
    /// One place builds this string, so the `Bearer ` prefix cannot be
    /// forgotten on one transport and remembered on another — which is
    /// exactly what the account websocket needs, since its `auth-token` field
    /// takes the same prefixed form as the HTTP header.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

/// One permission an authorization request asks the customer to grant.
///
/// A closed set: tastytrade documents exactly these three, and a scope the
/// venue does not know is rejected at the authorization page rather than at
/// the token endpoint, which is a much worse place to find out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// Read account data. Sensitive: the customer is asked for 2FA.
    Read,
    /// Place and manage orders. Sensitive: the customer is asked for 2FA.
    Trade,
    /// OpenID Connect. Adds `id_token` to the token response.
    OpenId,
}

impl Scope {
    /// The wire name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Trade => "trade",
            Scope::OpenId => "openid",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A request to send a customer to tastytrade's authorization page.
///
/// Only useful to a **trusted third-party** application: a personal
/// application is restricted to its own owner's account and uses
/// [`RefreshToken`] directly. Build one, send the customer to
/// [`AuthorizationRequest::authorize_url`], and exchange the code that comes
/// back with [`crate::TastyTrade::connect_with_authorization_code`].
///
/// Carries no secret. The client secret belongs in the token request and
/// nowhere near a URL a browser will keep in its history.
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    /// The public identifier of the OAuth application.
    pub client_id: String,
    /// Where the customer is sent back to. Must match one registered with
    /// tastytrade, and must be a full URI.
    pub redirect_uri: String,
    /// What the application is asking for. An empty list sends no `scope`
    /// parameter and lets the venue apply its default.
    pub scopes: Vec<Scope>,
    /// Opaque value echoed back on the redirect.
    ///
    /// The only defence against a redirect the application did not start, so
    /// [`AuthorizationRequest::verify_state`] refuses a response that does not
    /// carry it back exactly.
    pub state: Option<String>,
}

impl AuthorizationRequest {
    /// A request for `client_id` redirecting to `redirect_uri`.
    pub fn new(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
            scopes: Vec::new(),
            state: None,
        }
    }

    /// Asks for `scopes`.
    #[must_use]
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = Scope>) -> Self {
        self.scopes = scopes.into_iter().collect();
        self
    }

    /// Sends `state` and requires it back.
    ///
    /// The value must be unguessable and tied to the browser session that
    /// started the flow; this crate does not generate one, because a
    /// library-generated nonce the application cannot correlate is a nonce
    /// that proves nothing.
    #[must_use]
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// The URL to send the customer to.
    ///
    /// Every parameter is percent-encoded by construction, so a redirect URI
    /// with a query string of its own survives the round trip. `response_type`
    /// is always `code`: it is the only flow tastytrade documents.
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Precondition`] when the client id or redirect URI is
    /// blank, or the authorization host cannot be parsed — all of which are
    /// caller mistakes, caught before a customer is sent anywhere.
    pub fn authorize_url(&self, environment: Environment) -> TastyResult<String> {
        if self.client_id.trim().is_empty() {
            return Err(TastyTradeError::Precondition(
                "an authorization request needs a client id".to_string(),
            ));
        }
        if self.redirect_uri.trim().is_empty() {
            return Err(TastyTradeError::Precondition(
                "an authorization request needs a redirect URI registered with tastytrade"
                    .to_string(),
            ));
        }

        let host = match environment {
            Environment::Production => AUTHORIZE_URL,
            Environment::Certification => AUTHORIZE_DEMO_URL,
        };

        let mut params: Vec<(&str, String)> = vec![
            ("client_id", self.client_id.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("response_type", "code".to_string()),
        ];
        if !self.scopes.is_empty() {
            let scopes = self
                .scopes
                .iter()
                .map(Scope::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            params.push(("scope", scopes));
        }
        if let Some(state) = &self.state {
            params.push(("state", state.clone()));
        }

        reqwest::Url::parse_with_params(host, &params)
            .map(|url| url.to_string())
            .map_err(|e| {
                TastyTradeError::Precondition(format!("could not build the authorization URL: {e}"))
            })
    }

    /// Checks the `state` a redirect came back with.
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Precondition`] when a state was sent and the
    /// response does not echo it exactly. Nothing has been exchanged at that
    /// point, so refusing costs the customer one retry and refusing to refuse
    /// costs them an account.
    pub fn verify_state(&self, returned: Option<&str>) -> TastyResult<()> {
        match (&self.state, returned) {
            (None, _) => Ok(()),
            (Some(expected), Some(actual)) if expected == actual => Ok(()),
            (Some(_), Some(_)) => Err(TastyTradeError::Precondition(
                "the authorization response carried a different state than the request; \
                 discard the code rather than exchanging it"
                    .to_string(),
            )),
            (Some(_), None) => Err(TastyTradeError::Precondition(
                "the authorization response carried no state, but the request sent one; \
                 discard the code rather than exchanging it"
                    .to_string(),
            )),
        }
    }
}

/// Which OAuth2 grant a token request is making.
///
/// The two documented flows differ in more than a parameter: the refresh grant
/// can be replayed forever, and the authorization-code grant is single-use and
/// only reachable by a trusted third party. Making the choice a type means a
/// caller cannot half-fill one and send the other.
#[derive(Debug, Clone)]
pub enum OAuthGrant {
    /// `grant_type=refresh_token`. The personal flow, and what every later
    /// refresh uses whichever grant started the session.
    Refresh {
        /// The application's secret.
        client_secret: ClientSecret,
        /// The grant's refresh token.
        refresh_token: RefreshToken,
    },
    /// `grant_type=authorization_code`. Exchanges the single-use code from a
    /// redirect for a token pair.
    AuthorizationCode {
        /// The code the redirect carried back.
        code: AuthorizationCode,
        /// The application's public identifier.
        client_id: String,
        /// The application's secret.
        client_secret: ClientSecret,
        /// The same redirect URI the authorization request used. The venue
        /// compares them.
        redirect_uri: String,
    },
}

impl OAuthGrant {
    /// The form parameters for this grant.
    ///
    /// `application/x-www-form-urlencoded`, as RFC 6749 §4.1.3 and §6 require.
    /// The values are secrets, so this is one of the few places
    /// `expose_secret` is called; the result goes straight into a request body
    /// and is never logged.
    pub(crate) fn form_parameters(&self) -> Vec<(&'static str, &str)> {
        match self {
            OAuthGrant::Refresh {
                client_secret,
                refresh_token,
            } => vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.expose_secret()),
                ("client_secret", client_secret.expose_secret()),
            ],
            OAuthGrant::AuthorizationCode {
                code,
                client_id,
                client_secret,
                redirect_uri,
            } => vec![
                ("grant_type", "authorization_code"),
                ("code", code.expose_secret()),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.expose_secret()),
                ("redirect_uri", redirect_uri.as_str()),
            ],
        }
    }

    /// The grant type, for logs and errors. Never a secret.
    pub(crate) fn grant_type(&self) -> &'static str {
        match self {
            OAuthGrant::Refresh { .. } => "refresh_token",
            OAuthGrant::AuthorizationCode { .. } => "authorization_code",
        }
    }
}

/// A successful answer from `POST /oauth/token`.
///
/// `Debug` and `Display` redact all three tokens. Every field except
/// `expires_in` and `token_type` is a credential.
#[derive(Deserialize)]
pub struct TokenResponse {
    /// The bearer token to present on every request.
    pub access_token: AccessToken,
    /// A refresh token. Present on the authorization-code grant; the refresh
    /// grant does not have to send one back, and tastytrade's does not change.
    #[serde(default)]
    pub refresh_token: Option<RefreshToken>,
    /// Always `Bearer` in practice. Not a secret.
    #[serde(default)]
    pub token_type: Option<String>,
    /// Seconds the access token stays valid, about 900.
    #[serde(default)]
    pub expires_in: Option<u64>,
    /// The OpenID Connect identity token, for the `openid` scope only.
    #[serde(default)]
    pub id_token: Option<IdToken>,
}

impl TokenResponse {
    /// How long the access token is good for.
    ///
    /// Falls back to the documented fifteen minutes when the venue omits
    /// `expires_in`, which RFC 6749 §5.1 allows it to do. Guessing short is
    /// the safe direction: an unnecessary refresh costs one request, and a
    /// token believed live past its expiry costs a 401 mid-flow.
    ///
    /// Clamped to twenty-four hours. The value comes from the endpoint, and an
    /// unbounded one would panic the moment it was added to an `Instant`.
    pub fn lifetime(&self) -> Duration {
        self.expires_in
            .map(Duration::from_secs)
            .map(|lifetime| lifetime.min(MAX_TOKEN_LIFETIME))
            .unwrap_or(DEFAULT_TOKEN_LIFETIME)
    }
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &REDACTED)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| REDACTED),
            )
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("id_token", &self.id_token.as_ref().map(|_| REDACTED))
            .finish()
    }
}

impl fmt::Display for TokenResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// An access token together with the moment it stops being usable.
#[derive(Debug, Clone)]
pub(crate) struct ActiveToken {
    pub(crate) token: AccessToken,
    /// Monotonic on purpose: a system clock that steps backwards would extend
    /// a token's apparent life, and the venue does not care what this process
    /// thinks the time is.
    expires_at: Instant,
}

impl ActiveToken {
    /// A token that expires `lifetime` from now.
    ///
    /// `checked_add` rather than `+`: the caller is expected to have clamped
    /// the lifetime, but this is the line that would panic if it had not, and
    /// a panic here happens inside the caller's `connect`. An overflow falls
    /// back to the documented lifetime, which is short and therefore safe.
    pub(crate) fn new(token: AccessToken, lifetime: Duration) -> Self {
        let expires_at = Instant::now()
            .checked_add(lifetime)
            .unwrap_or_else(|| Instant::now() + DEFAULT_TOKEN_LIFETIME);

        Self { token, expires_at }
    }

    /// Whether this token is inside the refresh margin, or already past it.
    pub(crate) fn is_stale(&self) -> bool {
        self.remaining().map(|left| left <= REFRESH_MARGIN) != Some(false)
    }

    /// How much life the token has left, when it has any.
    pub(crate) fn remaining(&self) -> Option<Duration> {
        self.expires_at.checked_duration_since(Instant::now())
    }
}

/// Every error code RFC 6749 defines for the token and authorization
/// endpoints, and nothing else.
///
/// The list is the whitelist: a code outside it never reaches a log or an
/// error value.
const SPECIFIED_ERROR_CODES: [&str; 10] = [
    "invalid_request",
    "invalid_client",
    "invalid_grant",
    "unauthorized_client",
    "unsupported_grant_type",
    "invalid_scope",
    "access_denied",
    "unsupported_response_type",
    "server_error",
    "temporarily_unavailable",
];

/// Written instead of an error code the spec does not define.
const UNRECOGNISED_ERROR_CODE: &str = "an unrecognised error code";

/// What the venue said when it refused a token request.
///
/// RFC 6749 §5.2 defines the codes; the accompanying `error_description` is
/// free prose from an endpoint this crate does not control, so it is dropped
/// rather than carried into an error a caller will log.
///
/// `Debug` is manual and redacting. Deserialization enforces nothing about
/// what `error` contains, and this is the response to the one request whose
/// every parameter is a secret — an endpoint that echoed a client secret back
/// in that field would otherwise write it wherever this value went.
#[derive(Deserialize)]
pub(crate) struct TokenErrorResponse {
    error: String,
}

impl TokenErrorResponse {
    /// The code, when it is one the spec defines, and a constant label
    /// otherwise.
    ///
    /// `&'static str` on purpose: nothing venue-controlled can leave through
    /// this, whatever arrived in the body. It is the only accessor, so there
    /// is no second path.
    pub(crate) fn code(&self) -> &'static str {
        SPECIFIED_ERROR_CODES
            .into_iter()
            .find(|known| *known == self.error.trim())
            .unwrap_or(UNRECOGNISED_ERROR_CODE)
    }

    /// Whether this refusal is about the credentials rather than the request.
    ///
    /// A rejected credential is terminal: presenting it again produces the
    /// same answer, and both streamers treat [`TastyTradeError::Auth`] as the
    /// end of the line rather than something to back off and retry. An
    /// unrecognised code is **not** treated as terminal — giving up on a code
    /// this crate cannot read would turn an unexpected reply into a dead
    /// session.
    pub(crate) fn is_credential_failure(&self) -> bool {
        matches!(
            self.code(),
            "invalid_grant" | "invalid_client" | "unauthorized_client" | "access_denied"
        )
    }
}

impl fmt::Debug for TokenErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The sanitised code, never the field.
        f.debug_struct("TokenErrorResponse")
            .field("error", &self.code())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "SENTINEL-client-secret-3Qv7";
    const REFRESH: &str = "SENTINEL-refresh-token-8Hb2";
    const ACCESS: &str = "SENTINEL-access-token-5Nd9";

    /// Every secret-bearing type renders as `***`. This is the whole reason
    /// they are newtypes rather than `String`.
    #[test]
    fn secrets_never_render_themselves() {
        let rendered = format!(
            "{:?} {} {:?} {} {:?} {} {:?} {:?}",
            ClientSecret::new(SECRET),
            ClientSecret::new(SECRET),
            RefreshToken::new(REFRESH),
            RefreshToken::new(REFRESH),
            AccessToken::new(ACCESS),
            AccessToken::new(ACCESS),
            AuthorizationCode::new("SENTINEL-code-1Ww4"),
            IdToken::new("SENTINEL-id-token-2Ee5"),
        );

        for secret in [
            SECRET,
            REFRESH,
            ACCESS,
            "SENTINEL-code-1Ww4",
            "SENTINEL-id-token-2Ee5",
        ] {
            assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
        }
        assert!(rendered.contains(REDACTED), "{rendered}");
    }

    /// The response is the single richest secret in the crate: three tokens in
    /// one value.
    #[test]
    fn a_token_response_redacts_every_token() {
        let response = TokenResponse {
            access_token: AccessToken::new(ACCESS),
            refresh_token: Some(RefreshToken::new(REFRESH)),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(900),
            id_token: Some(IdToken::new("SENTINEL-id-token-2Ee5")),
        };

        for rendered in [format!("{response:?}"), format!("{response}")] {
            for secret in [ACCESS, REFRESH, "SENTINEL-id-token-2Ee5"] {
                assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
            }
            // The non-secret metadata survives, which is what an example is
            // allowed to print.
            assert!(rendered.contains("900"), "{rendered}");
            assert!(rendered.contains("Bearer"), "{rendered}");
        }
    }

    #[test]
    fn a_missing_expires_in_falls_back_to_the_documented_lifetime() {
        let response = TokenResponse {
            access_token: AccessToken::new(ACCESS),
            refresh_token: None,
            token_type: None,
            expires_in: None,
            id_token: None,
        };
        assert_eq!(response.lifetime(), DEFAULT_TOKEN_LIFETIME);

        let response = TokenResponse {
            expires_in: Some(42),
            ..response
        };
        assert_eq!(response.lifetime(), Duration::from_secs(42));
    }

    #[test]
    fn the_header_value_carries_the_bearer_prefix() {
        assert_eq!(AccessToken::new("abc").bearer(), "Bearer abc");
    }

    /// The authorization URL goes into a browser's address bar and its
    /// history. A client secret there is a client secret published.
    #[test]
    fn the_authorization_url_carries_no_secret_and_encodes_its_parameters() {
        let request =
            AuthorizationRequest::new("client-abc", "https://app.example.com/cb?flow=a b")
                .with_scopes([Scope::Read, Scope::Trade])
                .with_state("state-xyz");

        let url = request
            .authorize_url(Environment::Production)
            .expect("a complete request builds a URL");

        assert!(url.starts_with(AUTHORIZE_URL), "{url}");
        assert!(url.contains("response_type=code"), "{url}");
        assert!(url.contains("client_id=client-abc"), "{url}");
        assert!(url.contains("state=state-xyz"), "{url}");
        // Space and the nested query string both survive, encoded.
        assert!(
            url.contains("redirect_uri=https%3A%2F%2Fapp.example.com%2Fcb%3Fflow%3Da+b"),
            "{url}"
        );
        assert!(url.contains("scope=read+trade"), "{url}");
        assert!(!url.contains("client_secret"), "{url}");

        let sandbox = request
            .authorize_url(Environment::Certification)
            .expect("a complete request builds a URL");
        assert!(sandbox.starts_with(AUTHORIZE_DEMO_URL), "{sandbox}");
    }

    #[test]
    fn an_incomplete_authorization_request_fails_before_a_customer_is_sent_anywhere() {
        let error = AuthorizationRequest::new("  ", "https://app.example.com/cb")
            .authorize_url(Environment::Certification)
            .expect_err("a blank client id is not a request");
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "{error:?}"
        );

        let error = AuthorizationRequest::new("client-abc", "")
            .authorize_url(Environment::Certification)
            .expect_err("a blank redirect URI is not a request");
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "{error:?}"
        );
    }

    /// A redirect that does not echo the state is a redirect the application
    /// did not start.
    #[test]
    fn the_state_has_to_come_back_exactly() {
        let request =
            AuthorizationRequest::new("client-abc", "https://app.example.com/cb").with_state("s1");

        assert!(request.verify_state(Some("s1")).is_ok());
        assert!(request.verify_state(Some("s2")).is_err());
        assert!(request.verify_state(None).is_err());

        // Nothing sent, nothing to check.
        let stateless = AuthorizationRequest::new("client-abc", "https://app.example.com/cb");
        assert!(stateless.verify_state(None).is_ok());
        assert!(stateless.verify_state(Some("anything")).is_ok());
    }

    #[test]
    fn a_grant_renders_its_parameters_without_naming_them_in_its_type() {
        let refresh = OAuthGrant::Refresh {
            client_secret: ClientSecret::new(SECRET),
            refresh_token: RefreshToken::new(REFRESH),
        };
        assert_eq!(refresh.grant_type(), "refresh_token");
        let params = refresh.form_parameters();
        assert!(params.contains(&("grant_type", "refresh_token")));
        assert!(params.contains(&("refresh_token", REFRESH)));
        assert!(params.contains(&("client_secret", SECRET)));

        // The grant itself must not print what it holds.
        let rendered = format!("{refresh:?}");
        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(!rendered.contains(REFRESH), "{rendered}");

        let code = OAuthGrant::AuthorizationCode {
            code: AuthorizationCode::new("code-1"),
            client_id: "client-abc".to_string(),
            client_secret: ClientSecret::new(SECRET),
            redirect_uri: "https://app.example.com/cb".to_string(),
        };
        assert_eq!(code.grant_type(), "authorization_code");
        let params = code.form_parameters();
        assert!(params.contains(&("grant_type", "authorization_code")));
        assert!(params.contains(&("code", "code-1")));
        assert!(params.contains(&("redirect_uri", "https://app.example.com/cb")));
    }

    #[test]
    fn a_token_inside_the_margin_is_already_stale() {
        let fresh = ActiveToken::new(AccessToken::new(ACCESS), Duration::from_secs(900));
        assert!(!fresh.is_stale());
        assert!(fresh.remaining().is_some());

        let expiring = ActiveToken::new(AccessToken::new(ACCESS), REFRESH_MARGIN);
        assert!(
            expiring.is_stale(),
            "a token with only the margin left must be refreshed before it is used"
        );

        let expired = ActiveToken::new(AccessToken::new(ACCESS), Duration::ZERO);
        assert!(expired.is_stale());
        assert_eq!(expired.remaining(), None);
    }

    /// A rejected credential is terminal; a rejected request is not. Both
    /// streamers back off on one and give up on the other, so the
    /// classification decides whether a client hammers the venue.
    #[test]
    fn only_credential_refusals_are_terminal() {
        for code in [
            "invalid_grant",
            "invalid_client",
            "unauthorized_client",
            "access_denied",
        ] {
            assert!(
                refusal(code).is_credential_failure(),
                "{code} is about the credential"
            );
        }
        for code in ["invalid_request", "server_error", "temporarily_unavailable"] {
            assert!(
                !refusal(code).is_credential_failure(),
                "{code} is not about the credential"
            );
        }
    }

    fn refusal(code: &str) -> TokenErrorResponse {
        serde_json::from_str(&format!(r#"{{"error":"{code}"}}"#)).expect("a refusal document")
    }

    /// `error` is untrusted response text — deserialization enforces no
    /// grammar on it — and this is the reply to the one request whose every
    /// parameter is a secret. An endpoint that echoed a client secret back in
    /// that field must not get it into a log or an error value.
    #[test]
    fn an_error_code_the_spec_does_not_define_never_travels() {
        let refusal = refusal(SECRET);

        assert_eq!(
            refusal.code(),
            UNRECOGNISED_ERROR_CODE,
            "only the spec's own codes may leave this type"
        );
        assert!(
            !format!("{refusal:?}").contains(SECRET),
            "Debug rendered the field: {refusal:?}"
        );
        // And an unreadable code is not treated as a dead credential: giving
        // up on a reply this crate cannot parse would turn a surprise into a
        // terminated session.
        assert!(!refusal.is_credential_failure());
    }

    /// The whitelist is the whole mechanism, so it has to actually pass the
    /// codes through.
    #[test]
    fn a_specified_error_code_survives_intact() {
        for code in SPECIFIED_ERROR_CODES {
            assert_eq!(refusal(code).code(), code);
            // Surrounding whitespace is a transport artefact, not a different
            // code.
            assert_eq!(refusal(&format!("  {code} ")).code(), code);
        }
    }

    /// `expires_in` is a number the endpoint chooses, and `Instant + Duration`
    /// panics when the sum leaves the platform's range. A malformed but
    /// otherwise successful response must not abort the caller's process.
    #[test]
    fn an_absurd_lifetime_is_clamped_rather_than_panicking() {
        let response = TokenResponse {
            access_token: AccessToken::new(ACCESS),
            refresh_token: None,
            token_type: None,
            expires_in: Some(u64::MAX),
            id_token: None,
        };

        assert_eq!(
            response.lifetime(),
            MAX_TOKEN_LIFETIME,
            "an unbounded lifetime is not a lifetime"
        );

        // The construction that would have panicked, with the unclamped value
        // and with the clamped one.
        let token = ActiveToken::new(AccessToken::new(ACCESS), Duration::from_secs(u64::MAX));
        assert!(!token.is_stale(), "the fallback still has to be usable");
        assert!(
            ActiveToken::new(AccessToken::new(ACCESS), response.lifetime())
                .remaining()
                .is_some()
        );
    }

    /// A configuration file can supply a secret, which means the newtype has
    /// to deserialize from a plain string. It must not serialize back.
    #[test]
    fn a_secret_deserializes_transparently() {
        let secret: ClientSecret =
            serde_json::from_str("\"SENTINEL-client-secret-3Qv7\"").expect("a plain JSON string");
        assert_eq!(secret.expose_secret(), SECRET);
        assert!(!secret.is_blank());
        assert!(ClientSecret::new("  \t ").is_blank());
    }
}
