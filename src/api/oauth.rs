//! The OAuth2 session: token exchange, expiry and refresh.
//!
//! An access token lives about fifteen minutes, so authentication is not a
//! thing that happens once at startup any more — it happens continuously, and
//! quietly, underneath every request. [`OAuthSession`](crate::api::oauth::OAuthSession) owns that: it holds the
//! grant, remembers when the current token dies, and mints a new one before
//! anything tries to use a dead one.
//!
//! Two rules shape the design.
//!
//! **Refresh happens before a request, never after one fails.** A 401 on a
//! `POST /orders` is ambiguous — the order may have been placed — so there is
//! no path here that retries a mutating request with a fresh token. The token
//! is checked first; if the venue still says 401, that is an error the caller
//! sees.
//!
//! **A session is bound to the deployment it authenticated against.**
//! `TastyTradeConfig::base_url` is a public field on a struct callers build
//! themselves, and the client keeps its own clone of it. A token minted on
//! certification and presented to production would merely be refused, but a
//! *refresh* aimed at the wrong host is worse than a refusal: it is a client
//! secret sent somewhere the caller did not intend. The session records the
//! base URL it authenticated against and every verb checks it, so the two can
//! never drift apart unnoticed — least of all on the request that places an
//! order.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header;
use tokio::sync::Mutex;
use tracing::debug;

use crate::TastyTradeError;
use crate::api::base::TastyResult;
use crate::api::client::{RequestReport, transport_failure};
use crate::error::Environment;
use crate::types::oauth::{
    AccessToken, ActiveToken, AuthorizationCode, ClientSecret, OAuthGrant, RefreshToken,
    TokenErrorResponse, TokenResponse,
};

/// The one endpoint that mints access tokens, in both environments.
const TOKEN_PATH: &str = "/oauth/token";

/// Grant and current token, changed together.
///
/// One lock rather than two: a refresh reads the grant and writes the token,
/// and splitting them lets two callers exchange the same authorization code.
struct SessionState {
    /// What to present for the next refresh.
    ///
    /// `None` when the session cannot refresh itself: an authorization-code
    /// exchange that came back without a refresh token leaves the caller with
    /// a working access token and no way to renew it, which is worth saying
    /// once the token dies rather than pretending at startup.
    grant: Option<OAuthGrant>,
    /// The token in use, while it is still worth using.
    active: Option<ActiveToken>,
}

/// An authenticated OAuth2 session.
///
/// Cloned by every [`crate::TastyTrade`] clone through an `Arc`, so a refresh
/// performed by one clone is immediately visible to the others — including the
/// account streamer's supervisor, which is often the thing that notices a
/// token has expired first.
pub struct OAuthSession {
    http: reqwest::Client,
    /// The deployment this session authenticated against.
    base_url: String,
    environment: Environment,
    state: Mutex<SessionState>,
}

impl std::fmt::Debug for OAuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No token, no grant, no secret. The environment and the endpoint are
        // the useful part and the safe part.
        f.debug_struct("OAuthSession")
            .field("environment", &self.environment)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl OAuthSession {
    /// Authenticates `grant` against `base_url` and keeps it for refreshes.
    ///
    /// The first exchange happens here, so a caller with a bad secret learns
    /// at construction rather than on their first request.
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Auth`] when the venue refuses the credentials, which
    /// is terminal. Any other failure keeps its [`TastyTradeError::Request`]
    /// shape with the status, because a token endpoint that is down says
    /// nothing about whether the secret is good.
    pub(crate) async fn establish(
        http: reqwest::Client,
        base_url: &str,
        environment: Environment,
        grant: OAuthGrant,
    ) -> TastyResult<Arc<Self>> {
        let session = Arc::new(Self {
            http,
            base_url: base_url.to_string(),
            environment,
            state: Mutex::new(SessionState {
                grant: None,
                active: None,
            }),
        });

        let response = session.exchange(&grant).await?;
        let lifetime = response.lifetime();
        // Whatever grant started the session, every later renewal is a refresh:
        // an authorization code is single-use, and replaying one is how a
        // client turns a working session into a revoked grant.
        let next = match response.refresh_token.as_ref() {
            Some(refresh_token) => match &grant {
                OAuthGrant::Refresh { client_secret, .. }
                | OAuthGrant::AuthorizationCode { client_secret, .. } => {
                    Some(OAuthGrant::Refresh {
                        client_secret: client_secret.clone(),
                        refresh_token: refresh_token.clone(),
                    })
                }
            },
            // The refresh grant does not have to echo the refresh token back,
            // and tastytrade's does not change, so keep presenting the one we
            // already hold. An authorization-code exchange with no refresh
            // token genuinely cannot renew.
            None => match grant {
                OAuthGrant::Refresh { .. } => Some(grant),
                OAuthGrant::AuthorizationCode { .. } => None,
            },
        };

        {
            let mut state = session.state.lock().await;
            state.grant = next;
            state.active = Some(ActiveToken::new(response.access_token, lifetime));
        }

        debug!(
            "OAuth2 session established on {} (token valid for {}s)",
            environment,
            lifetime.as_secs()
        );

        Ok(session)
    }

    /// A token that will still be valid when the request that uses it lands.
    ///
    /// Refreshes when the current one is inside the margin. Concurrent callers
    /// serialise on the same lock, so a burst of requests arriving on an
    /// expired token produces one refresh, not one per request.
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Auth`] when the refresh is refused, or when the
    /// session has no way to refresh — an authorization-code exchange that
    /// returned no refresh token, once its access token has died.
    pub async fn access_token(&self) -> TastyResult<AccessToken> {
        let mut state = self.state.lock().await;

        if let Some(active) = &state.active
            && !active.is_stale()
        {
            return Ok(active.token.clone());
        }

        let Some(grant) = state.grant.clone() else {
            return Err(TastyTradeError::Auth(format!(
                "the access token for this {} session has expired and the session has no \
                 refresh token; authorize again to obtain one",
                self.environment
            )));
        };

        let response = self.exchange(&grant).await?;
        if let Some(refresh_token) = response.refresh_token.as_ref() {
            // The venue is allowed to rotate it. Keeping the old one would work
            // until it did not, at the worst possible moment.
            state.grant = Some(OAuthGrant::Refresh {
                client_secret: match &grant {
                    OAuthGrant::Refresh { client_secret, .. }
                    | OAuthGrant::AuthorizationCode { client_secret, .. } => client_secret.clone(),
                },
                refresh_token: refresh_token.clone(),
            });
        }

        let lifetime = response.lifetime();
        let token = response.access_token.clone();
        state.active = Some(ActiveToken::new(response.access_token, lifetime));
        debug!(
            "Refreshed the {} access token ({}s)",
            self.environment,
            lifetime.as_secs()
        );

        Ok(token)
    }

    /// How long the current access token has left.
    ///
    /// `None` when there is none or it has already expired. A duration is not
    /// a secret, so this is what an example prints instead of a token.
    pub async fn expires_in(&self) -> Option<Duration> {
        self.state
            .lock()
            .await
            .active
            .as_ref()
            .and_then(super::super::types::oauth::ActiveToken::remaining)
    }

    /// The refresh token this session would use next, if it has one.
    ///
    /// The one way a secret leaves this crate, and it exists for a single
    /// reason: an authorization-code exchange mints a refresh token the
    /// application has to store, and a session that kept it to itself would
    /// force the customer through the authorization page again on every
    /// restart.
    pub async fn refresh_token(&self) -> Option<RefreshToken> {
        match self.state.lock().await.grant.as_ref() {
            Some(OAuthGrant::Refresh { refresh_token, .. }) => Some(refresh_token.clone()),
            _ => None,
        }
    }

    /// Which deployment this session authenticated against.
    pub fn environment(&self) -> Environment {
        self.environment
    }

    /// Fails unless `base_url` is the one this session was established against.
    ///
    /// Checked on every verb rather than trusted once at construction. It
    /// costs a string comparison, and what it buys is that no future change to
    /// how a client gets its configuration can quietly point an authenticated
    /// session — or the refresh that carries the client secret — at a
    /// different deployment.
    ///
    /// # Errors
    ///
    /// [`TastyTradeError::Precondition`]. Nothing is sent.
    pub(crate) fn ensure_same_deployment(&self, base_url: &str) -> TastyResult<()> {
        if base_url == self.base_url {
            return Ok(());
        }
        Err(TastyTradeError::Precondition(format!(
            "this session authenticated against {} and the configuration now points somewhere \
             else; build a new client rather than moving an existing session between deployments",
            self.environment
        )))
    }

    /// One round trip to the token endpoint.
    ///
    /// Neither the request body nor the response body is logged at any level.
    /// The request body is the client secret and the response body is three
    /// tokens, so this is the single most credential-dense exchange in the
    /// crate — the usual "body at DEBUG" allowance has no place here.
    async fn exchange(&self, grant: &OAuthGrant) -> TastyResult<TokenResponse> {
        // The endpoint is fixed and carries no account, so there is nothing to
        // redact. The grant type is safe: it names the flow, not the secret.
        let report = RequestReport::new(
            "POST",
            format!("{TOKEN_PATH} ({})", grant.grant_type()),
            self.environment,
        );

        let response = self
            .http
            .post(format!("{}{TOKEN_PATH}", self.base_url))
            // RFC 6749 §4.1.3 and §6: the token endpoint takes form-encoded
            // parameters. `form` sets the content type itself, and the client
            // deliberately has no default Content-Type that would shadow it.
            .form(&grant.form_parameters())
            .send()
            .await
            .map_err(|e| transport_failure(&report, e))?;

        let status = response.status();
        let Ok(body) = response.text().await else {
            // The response arrived, so the status is real information even
            // though the body never turned up.
            debug!(
                "POST {TOKEN_PATH}: reading the body failed after {}",
                status
            );
            return Err(TastyTradeError::Request {
                context: report.context(Some(status.as_u16())),
                api: None,
            });
        };

        if !status.is_success() {
            // RFC 6749 §5.2 gives the refusal a machine-readable code and a
            // free-prose `error_description`. Only the code is kept: the prose
            // comes from an endpoint this crate does not control, on the one
            // request whose parameters are all secret.
            let parsed = serde_json::from_str::<TokenErrorResponse>(&body).ok();
            // `code()` answers with a `&'static str` from the spec's own list,
            // so nothing the endpoint chose to put in that field can travel
            // through here — not into this log line, and not into the error
            // below. Deserialization enforces no grammar on it, and this is
            // the one response whose neighbouring fields are all secret.
            let code = parsed
                .as_ref()
                .map(TokenErrorResponse::code)
                .unwrap_or("no error code");

            debug!(
                "POST {TOKEN_PATH} -> {} ({} bytes, {})",
                status.as_u16(),
                body.len(),
                code
            );

            let credentials_refused = parsed
                .as_ref()
                .is_some_and(TokenErrorResponse::is_credential_failure)
                || matches!(status.as_u16(), 401 | 403);

            return if credentials_refused {
                // Terminal. Both streamers stop on Auth rather than backing off,
                // because presenting the same secret again gets the same answer.
                Err(TastyTradeError::Auth(format!(
                    "the {} token endpoint refused the {} grant ({code})",
                    self.environment,
                    grant.grant_type()
                )))
            } else {
                // A token endpoint that is down says nothing about the secret,
                // so this keeps its request shape and stays retryable.
                Err(TastyTradeError::Request {
                    context: report.context(Some(status.as_u16())),
                    api: None,
                })
            };
        }

        debug!(
            "POST {TOKEN_PATH} -> {} ({} bytes in {:?})",
            status.as_u16(),
            body.len(),
            report.elapsed()
        );

        serde_json::from_str::<TokenResponse>(&body).map_err(|e| {
            // Not even at DEBUG. `serde_json`'s Display quotes the value it
            // rejected, and every value in this body is a token.
            debug!(
                "POST {TOKEN_PATH}: decode failed ({:?} at line {}, column {})",
                e.classify(),
                e.line(),
                e.column()
            );
            TastyTradeError::Request {
                context: report.context(Some(status.as_u16())),
                api: None,
            }
        })
    }
}

/// Builds the personal refresh-token grant from a secret and a refresh token.
pub(crate) fn refresh_grant(
    client_secret: ClientSecret,
    refresh_token: RefreshToken,
) -> OAuthGrant {
    OAuthGrant::Refresh {
        client_secret,
        refresh_token,
    }
}

/// Builds the third-party authorization-code grant.
pub(crate) fn authorization_code_grant(
    code: AuthorizationCode,
    client_id: String,
    client_secret: ClientSecret,
    redirect_uri: String,
) -> OAuthGrant {
    OAuthGrant::AuthorizationCode {
        code,
        client_id,
        client_secret,
        redirect_uri,
    }
}

/// The default headers every tastytrade request carries.
///
/// `User-Agent` is not optional and not free-form: the venue rejects a request
/// without one, and the OAuth guide requires the `<product>/<version>` shape
/// specifically. The old value was the bare word `tastytrade`, which has no
/// version and is exactly the form that gets refused.
pub(crate) fn default_headers() -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(concat!("tastytrade-rs/", env!("CARGO_PKG_VERSION"))),
    );
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The venue rejects a `User-Agent` that is not `<product>/<version>`, and
    /// rejects a missing one outright. This is the header the whole crate
    /// depends on and the easiest one to break without noticing.
    #[test]
    fn the_user_agent_carries_a_version() {
        let headers = default_headers();
        let agent = headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .expect("a user agent is not optional");

        let (product, version) = agent
            .split_once('/')
            .expect("the venue requires <product>/<version>");
        assert!(!product.is_empty(), "{agent}");
        assert!(
            version.chars().next().is_some_and(|c| c.is_ascii_digit()),
            "the version must look like one: {agent}"
        );
    }

    /// No Content-Type default, on purpose: `RequestBuilder::form` only sets
    /// the form content type when nothing has claimed it, so a JSON default
    /// here would silently send the token request as JSON.
    #[test]
    fn no_default_content_type_can_shadow_the_form_encoding() {
        assert!(default_headers().get(header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn a_session_debug_shows_where_but_not_what() {
        let session = OAuthSession {
            http: reqwest::Client::new(),
            base_url: "https://api.cert.tastyworks.com".to_string(),
            environment: Environment::Certification,
            state: Mutex::new(SessionState {
                grant: Some(refresh_grant(
                    "SENTINEL-client-secret-3Qv7".into(),
                    "SENTINEL-refresh-token-8Hb2".into(),
                )),
                active: None,
            }),
        };

        let rendered = format!("{session:?}");
        assert!(rendered.contains("Certification"), "{rendered}");
        assert!(!rendered.contains("SENTINEL"), "{rendered}");
    }

    /// A session that followed its configuration to another deployment would
    /// send the client secret to a host the caller never authenticated
    /// against. Refusing costs them a rebuild.
    #[test]
    fn a_session_refuses_to_move_between_deployments() {
        let session = OAuthSession {
            http: reqwest::Client::new(),
            base_url: "https://api.cert.tastyworks.com".to_string(),
            environment: Environment::Certification,
            state: Mutex::new(SessionState {
                grant: None,
                active: None,
            }),
        };

        assert!(
            session
                .ensure_same_deployment("https://api.cert.tastyworks.com")
                .is_ok()
        );

        let error = session
            .ensure_same_deployment("https://api.tastyworks.com")
            .expect_err("a session must not follow the configuration to production");
        assert!(
            matches!(error, TastyTradeError::Precondition(_)),
            "nothing was sent, so this is a precondition: {error:?}"
        );
    }

    /// An authorization-code session with no refresh token works until its
    /// access token dies and then says why, rather than looping on a refresh
    /// it cannot make.
    #[tokio::test]
    async fn a_session_that_cannot_refresh_says_so_when_the_token_dies() {
        let session = OAuthSession {
            http: reqwest::Client::new(),
            base_url: "http://127.0.0.1:1".to_string(),
            environment: Environment::Certification,
            state: Mutex::new(SessionState {
                grant: None,
                active: Some(ActiveToken::new(
                    "SENTINEL-access-token-5Nd9".into(),
                    Duration::ZERO,
                )),
            }),
        };

        let error = session
            .access_token()
            .await
            .expect_err("an expired token with no grant cannot be renewed");
        assert!(matches!(error, TastyTradeError::Auth(_)), "{error:?}");
        assert!(!error.is_retryable(), "asking again cannot help");
        assert!(!format!("{error}").contains("SENTINEL"), "{error}");
    }

    /// A live token is handed back without touching the network. The base URL
    /// here is unroutable on purpose: if this ever tried to refresh, the test
    /// would fail rather than pass slowly.
    #[tokio::test]
    async fn a_live_token_is_reused() {
        let session = OAuthSession {
            http: reqwest::Client::new(),
            base_url: "http://127.0.0.1:1".to_string(),
            environment: Environment::Certification,
            state: Mutex::new(SessionState {
                grant: None,
                active: Some(ActiveToken::new(
                    "SENTINEL-access-token-5Nd9".into(),
                    Duration::from_secs(900),
                )),
            }),
        };

        let token = session
            .access_token()
            .await
            .expect("the token is still live");
        assert_eq!(token.expose_secret(), "SENTINEL-access-token-5Nd9");
        assert!(session.expires_in().await.is_some());
        assert!(session.refresh_token().await.is_none());
    }
}
