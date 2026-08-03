use std::fmt::Display;
use std::sync::Arc;

use crate::accounts::AccountDetails;
use crate::accounts::{Account, AccountInner, AccountNumber};
use crate::api::base::Items;
use crate::api::base::Paginated;
use crate::api::base::Response;
use crate::api::base::TastyApiResponse;
use crate::api::base::TastyResult;
use crate::api::oauth::{OAuthSession, authorization_code_grant, default_headers, refresh_grant};
use crate::api::query::{PageRequest, QueryBuilder};
use crate::api::url::encode_path_segment;
use crate::error::{ApiError, InnerApiError};
use crate::streaming::quote_streamer::QuoteStreamer;
use crate::types::customer::Customer;
use crate::types::margin::{MarginConfiguration, SpanExchange, SpanRow};
use crate::types::market_data::{MarketDataRequest, MarketDataSnapshot};
use crate::types::market_metrics::{
    DividendReport, EarningsRange, EarningsReport, MarketMetric, symbols_query,
};
use crate::types::oauth::{AccessToken, AuthorizationCode, RefreshToken};
use crate::types::order::LiveOrderRecord;
use crate::types::order_filter::{CustomerLiveOrderFilter, CustomerOrderFilter};
use crate::types::quote_alert::{NewQuoteAlert, QuoteAlert};
use crate::types::watchlist::{NewWatchlist, PairsWatchlist, Watchlist};
use crate::utils::config::TastyTradeConfig;
use chrono::NaiveDate;
use reqwest::ClientBuilder;
use reqwest::header;
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::debug;

/// An authenticated tastytrade client.
///
/// Authentication is OAuth2: the client holds a session that mints short-lived
/// bearer tokens and renews them underneath every request. Cloning is cheap
/// and shares both the HTTP client and the session, so a refresh performed by
/// one clone is seen by all of them.
///
/// `Debug` shows the environment and the configuration, neither of which
/// carries a secret.
#[derive(Clone)]
pub struct TastyTrade {
    pub(crate) client: reqwest::Client,
    pub(crate) session: Arc<OAuthSession>,
    pub(crate) config: TastyTradeConfig,
}

impl std::fmt::Debug for TastyTrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TastyTrade")
            .field("session", &self.session)
            .field("config", &self.config)
            .finish()
    }
}

impl Display for TastyTrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TastyTrade")
    }
}

/// Replaces the identifiers an API path carries with placeholders.
///
/// Every `/accounts/{number}/…` request embeds the account number in its path,
/// and every `/customers/{id}/…` request embeds the customer identifier. That
/// URL is useful context in an error, the identifiers are not: an error value
/// is logged, reported, or shown wherever the caller decides, so it must not
/// carry something the caller never chose to handle.
///
/// `/customers/me` is left alone. It is a literal the venue defines rather than
/// an identifier, it is by far the most common form, and redacting it would
/// make the ordinary path unreadable while hiding nothing.
///
/// The **query string** carries them too. The customer order endpoints take
/// `account-numbers[]` as a repeated parameter, so redacting only the path left
/// the identifiers in `RequestReport.operation` — and from there in every error
/// the request produced. The parameter names are matched by
/// [`crate::types::wire::names_an_account`], the same rule the JSON renderer
/// uses, so a new spelling is handled in one place rather than two.
fn redact_account_path(url: &str) -> String {
    let (path, query) = match url.find('?') {
        Some(at) => (&url[..at], Some(&url[at + 1..])),
        None => (url, None),
    };

    let mut out = redact_path_identifiers(path);
    if let Some(query) = query {
        out.push('?');
        out.push_str(&redact_query_identifiers(query));
    }

    out
}

/// The path half of [`redact_account_path`].
fn redact_path_identifiers(path: &str) -> String {
    /// What the next segment is an identifier *of*, when it is one.
    enum Next {
        Nothing,
        Account,
        Customer,
    }

    let mut out = String::with_capacity(path.len());
    let mut next = Next::Nothing;

    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            out.push('/');
        }
        // A fragment would end the path the way a query does. The verbs never
        // build one, but the redaction is what stands between a URL and a log
        // line, so it does not assume that.
        let (identifier, tail) = match segment.find('#') {
            Some(at) => segment.split_at(at),
            None => (segment, ""),
        };

        match next {
            Next::Account if !identifier.is_empty() => {
                out.push_str("{account}");
                out.push_str(tail);
                next = Next::Nothing;
                continue;
            }
            Next::Customer if !identifier.is_empty() && identifier != "me" => {
                out.push_str("{customer}");
                out.push_str(tail);
                next = Next::Nothing;
                continue;
            }
            _ => {}
        }

        out.push_str(segment);
        next = match identifier {
            "accounts" => Next::Account,
            "customers" => Next::Customer,
            _ => Next::Nothing,
        };
    }

    out
}

/// The query half of [`redact_account_path`].
///
/// Only the value is replaced. The parameter names are what say which request
/// was made, and an error that cannot name the request it failed on is worse
/// than one that says too much.
fn redact_query_identifiers(query: &str) -> String {
    query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if crate::types::wire::names_an_account(key) => {
                format!("{key}={{account}}")
            }
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Joins the configured base URL and a request path.
///
/// The verbs take a path, not a URL. Passing an absolute one produced
/// `http://host:8080http://elsewhere/...`, which fails to parse and surfaces
/// as a transport error with no status and nothing pointing at the cause. It
/// is a caller mistake, so it is reported as one, before anything is sent.
fn endpoint_url(base_url: &str, path: &str) -> TastyResult<String> {
    let lowered = path.trim_start().to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return Err(crate::TastyTradeError::Precondition(format!(
            "expected a path such as \"/accounts\", got an absolute URL; \
             the base URL comes from the configuration and decides which \
             deployment the request reaches (redacted path: {})",
            redact_account_path(path)
        )));
    }

    if let Some(segment) = path
        .split(['?', '#'])
        .next()
        .unwrap_or(path)
        .split('/')
        .find(|segment| is_dot_segment(segment))
    {
        return Err(crate::TastyTradeError::Precondition(format!(
            "path segment {segment:?} is a relative-reference marker rather than a value; \
             URL resolution removes it before the request is sent, so the request would \
             reach a different endpoint than the one asked for (redacted path: {})",
            redact_account_path(path)
        )));
    }

    Ok(format!("{base_url}{path}"))
}

/// Whether a segment is one URL resolution consumes instead of sending.
///
/// `.` and `..` are not ordinary segments. RFC 3986 §5.2.4 and the WHATWG URL
/// Standard both remove them while resolving a reference, and the WHATWG rules
/// treat the percent-encoded spellings — `%2e`, `%2E`, `.%2e`, `%2e.` — exactly
/// the same way. So there is no encoding of a segment that is *entirely* a dot
/// or two which survives to the wire: `/instruments/equities/..` is sent as
/// `/instruments/`, and `/instruments/equities/.` as `/instruments/equities/`.
/// Both are real endpoints that answer successfully, which is why this fails
/// silently rather than loudly.
///
/// [`super::url::encode_path_segment`] cannot fix it, since the escape it would
/// produce is one of the spellings the standard folds back. So the segment is
/// refused here, where every path already passes through on its way to a URL.
///
/// A dot *inside* a longer segment is untouched: `BRK.B` and the leading `./`
/// of a future option symbol are values, not markers.
fn is_dot_segment(segment: &str) -> bool {
    if segment.len() > 6 {
        return false;
    }
    let decoded = segment.replace("%2e", ".").replace("%2E", ".");
    decoded == "." || decoded == ".."
}

/// What a request needs in order to report itself without leaking anything.
///
/// Built once per request and carried through both the transport failure and
/// the decode paths, so every verb produces the same shape and no verb can
/// forget a field.
pub(crate) struct RequestReport {
    method: &'static str,
    /// Already redacted: no account number reaches this.
    operation: String,
    environment: crate::error::Environment,
    /// Stamped when the report is built, which is before the request goes out,
    /// so the observable timing covers the whole exchange rather than the part
    /// after the headers arrived.
    started: std::time::Instant,
}

impl RequestReport {
    /// A report for `method operation`, timed from now.
    pub(crate) fn new(
        method: &'static str,
        operation: String,
        environment: crate::error::Environment,
    ) -> Self {
        Self {
            method,
            operation,
            environment,
            started: std::time::Instant::now(),
        }
    }

    pub(crate) fn context(&self, status: Option<u16>) -> crate::error::RequestContext {
        crate::error::RequestContext {
            method: self.method,
            operation: self.operation.clone(),
            environment: self.environment,
            status,
        }
    }

    /// How long the exchange has taken so far. A duration, never contents.
    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Turns one response into a value or a typed error.
///
/// The single place status handling and decoding happen, so every verb reports
/// failures the same way. GET used to do this properly and POST, DELETE and
/// login each did something different: none of the other three inspected the
/// status at all, so a rejected login surfaced as a decode failure with no
/// status, no endpoint and no environment — the least diagnosable error in the
/// crate, on the request most worth diagnosing.
async fn decode_response<T, R>(
    report: &RequestReport,
    response: reqwest::Response,
) -> TastyResult<R>
where
    T: DeserializeOwned + Serialize + std::fmt::Debug,
    R: FromTastyResponse<T>,
{
    let status = response.status();
    // The status is known before the body is: a response whose body fails
    // mid-read still answered, and that answer is what a caller retries on.
    // Routing this through transport_failure would report `None` and lose it.
    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            debug!(
                "{} {}: reading the body failed after {}: {}",
                report.method,
                report.operation,
                status.as_u16(),
                e.without_url()
            );
            return Err(crate::TastyTradeError::Request {
                context: report.context(Some(status.as_u16())),
                api: None,
            });
        }
    };

    if !status.is_success() {
        // The body is not logged at any level. An error response comes from an
        // endpoint this code does not control, /sessions included, so it can
        // echo a credential; the secrecy invariant has no DEBUG exemption.
        // What is safe is the shape of it.
        let parsed = serde_json::from_str::<TastyApiResponse<serde_json::Value>>(&body);
        debug!(
            "{} {} -> {} ({} bytes in {:?}, {})",
            report.method,
            report.operation,
            status.as_u16(),
            body.len(),
            report.started.elapsed(),
            match &parsed {
                Ok(TastyApiResponse::Error { .. }) => "broker error document",
                Ok(TastyApiResponse::Success(_)) => "success envelope on a failure status",
                Err(_) => "unrecognised body",
            }
        );

        return Err(crate::TastyTradeError::Request {
            context: report.context(Some(status.as_u16())),
            api: match parsed {
                Ok(TastyApiResponse::Error { error }) => Some(sanitize_api_error(error)),
                _ => None,
            },
        });
    }

    // Size and timing, never contents: this is the line that makes an exchange
    // observable without making it quotable.
    debug!(
        "{} {} -> {} ({} bytes in {:?})",
        report.method,
        report.operation,
        status.as_u16(),
        body.len(),
        report.started.elapsed()
    );

    match serde_json::from_str::<TastyApiResponse<T>>(&body) {
        Ok(TastyApiResponse::Success(s)) => R::from_tasty(s),
        // A 2xx carrying an error document. The venue disagrees with itself,
        // and the document is the more specific answer.
        Ok(TastyApiResponse::Error { error }) => Err(crate::TastyTradeError::Request {
            context: report.context(Some(status.as_u16())),
            api: Some(sanitize_api_error(error)),
        }),
        Err(e) => {
            // The body stays out of the error: a caller that logs or reports it
            // would leak account data they never asked to handle.
            //
            // The error is not rendered either, at any level. `serde_json`
            // quotes the value it rejected ("invalid type: integer `12345`"),
            // so its Display can be a fragment of the body. Today the untagged
            // `TastyApiResponse` discards the inner error and reports only
            // "data did not match any variant", which masks the value — but
            // that is a property of the envelope, not a guarantee of this line,
            // and it disappears the day the envelope becomes tagged. Category
            // and position say what to look at without saying what it holds.
            debug!(
                "{} {}: decode failed ({:?} at line {}, column {})",
                report.method,
                report.operation,
                e.classify(),
                e.line(),
                e.column()
            );
            Err(crate::TastyTradeError::Request {
                context: report.context(Some(status.as_u16())),
                api: None,
            })
        }
    }
}

/// Wraps a transport failure in the same sanitised shape as a venue failure.
///
/// The reqwest error itself is kept out of the value: its `Display` renders
/// the URL it was trying to reach, account number included.
pub(crate) fn transport_failure(
    report: &RequestReport,
    error: reqwest::Error,
) -> crate::TastyTradeError {
    // `without_url` strips it for the log too. The redacted operation is
    // already there, and it is the part that says what failed.
    debug!(
        "transport failure on {} {}: {}",
        report.method,
        report.operation,
        error.without_url()
    );
    crate::TastyTradeError::Request {
        context: report.context(None),
        api: None,
    }
}

/// Strips the parts of a broker error document that can carry account data.
///
/// The top-level code and message are the broker's summary of what went wrong
/// and are what a caller acts on. The nested entries are per-field detail, and
/// that detail is where balances, buying power and account references show up
/// ("needs 1234567.89 buying power"). Their codes identify the failing rule
/// perfectly well without the numbers.
///
/// `ApiError` renders itself as JSON in both `Display` and `Debug`, so
/// anything left in the value is reachable from either.
fn sanitize_api_error(error: ApiError) -> ApiError {
    ApiError {
        code: error.code,
        message: error.message,
        errors: error.errors.map(|inner| {
            inner
                .into_iter()
                .map(|entry| InnerApiError {
                    code: entry.code,
                    message: "<redacted: enable DEBUG on the caller side>".to_string(),
                })
                .collect()
        }),
    }
}

/// Converts a decoded response envelope into the value a caller asked for.
///
/// Fallible because not every envelope can produce every target: a paginated
/// result needs a pagination block the venue may not have sent, and a library
/// must report that rather than panic in the caller's process.
pub trait FromTastyResponse<T: DeserializeOwned + Serialize + std::fmt::Debug>: Sized {
    /// Builds `Self` from `resp`, or explains why it cannot.
    fn from_tasty(resp: Response<T>) -> TastyResult<Self>;
}

impl<T: DeserializeOwned + Serialize + std::fmt::Debug> FromTastyResponse<T> for T {
    fn from_tasty(resp: Response<T>) -> TastyResult<Self> {
        Ok(resp.data)
    }
}

impl<T: DeserializeOwned + Serialize + std::fmt::Debug> FromTastyResponse<Items<T>>
    for Paginated<T>
{
    fn from_tasty(resp: Response<Items<T>>) -> TastyResult<Self> {
        // The venue decides whether a listing paginates. Asking for a
        // Paginated<T> from a response without a pagination block is a
        // mismatch the caller can act on, not a reason to abort their process.
        let Some(pagination) = resp.pagination else {
            // context is venue-provided and mirrors the request path, so it
            // carries the account number on account-scoped endpoints. Same
            // redaction as everywhere else.
            return Err(crate::TastyTradeError::Unknown(format!(
                "response for {} carried no pagination block; \
                 request this endpoint as a plain listing instead",
                redact_account_path(&resp.context)
            )));
        };

        // Counts and offsets describe the shape of the page, not its contents.
        debug!(
            "paginated page: {} items decoded, {} in page, {} total, offset {}",
            resp.data.items.len(),
            pagination.current_item_count,
            pagination.total_items,
            pagination.page_offset
        );

        Ok(Paginated {
            items: resp.data.into_items()?,
            pagination,
        })
    }
}

impl TastyTrade {
    /// Authenticates using configuration read from the environment.
    ///
    /// # Errors
    ///
    /// Fails without a network request when the OAuth credentials are missing,
    /// and with the venue's own answer when they are rejected. Certification is
    /// the default environment; see
    /// [`crate::utils::config::TastyTradeConfig`].
    pub async fn from_env() -> TastyResult<Self> {
        let config = TastyTradeConfig::from_env();
        Self::connect(&config).await
    }

    /// Authenticates with the personal refresh-token grant.
    ///
    /// This is the flow for an OAuth application you created for yourself on
    /// `my.tastytrade.com`: the configuration supplies the client secret and
    /// the grant's refresh token, and this exchanges them for the first access
    /// token. The token is renewed automatically from then on.
    ///
    /// # Errors
    ///
    /// [`crate::TastyTradeError::ConfigError`] without contacting the venue
    /// when the client secret or refresh token is missing or blank. The error
    /// names the variables to set and never their values.
    ///
    /// Credentials the venue refuses return [`crate::TastyTradeError::Auth`],
    /// which is not retryable. Any other failure keeps the
    /// [`crate::TastyTradeError::Request`] shape with its status, because a
    /// token endpoint that is down says nothing about whether the secret is
    /// good. The request body carries the client secret and the response body
    /// carries the tokens, so neither reaches the error or the logs at any
    /// level.
    pub async fn connect(config: &TastyTradeConfig) -> TastyResult<Self> {
        // Fail here rather than posting an empty credential pair to the venue.
        // The message names the variables, never their values.
        if !config.has_valid_credentials() {
            return Err(crate::TastyTradeError::ConfigError(
                "Missing tastytrade OAuth credentials: set TASTYTRADE_CLIENT_SECRET and \
                 TASTYTRADE_REFRESH_TOKEN, or load a configuration file that provides both. \
                 Create them under Manage > My Profile > API on my.tastytrade.com"
                    .to_string(),
            ));
        }

        Self::establish(
            config,
            refresh_grant(config.client_secret.clone(), config.refresh_token.clone()),
        )
        .await
    }

    /// Exchanges an authorization code for a session.
    ///
    /// The trusted third-party flow: after a customer authorizes the
    /// application at [`crate::oauth::AuthorizationRequest::authorize_url`]
    /// and is redirected back with a `code`, this exchanges it. Check the
    /// returned `state` with
    /// [`crate::oauth::AuthorizationRequest::verify_state`] **before**
    /// calling this — a code from a redirect the application did not start is
    /// not worth exchanging.
    ///
    /// The response carries a refresh token that never expires. Store it with
    /// [`TastyTrade::refresh_token`] and use [`TastyTrade::connect`] next time,
    /// rather than sending the customer through the authorization page again.
    ///
    /// # Errors
    ///
    /// [`crate::TastyTradeError::ConfigError`] without contacting the venue
    /// when the client id, client secret or redirect URI is missing. A code the
    /// venue refuses — expired, already used, or issued for another
    /// application — returns [`crate::TastyTradeError::Auth`].
    pub async fn connect_with_authorization_code(
        config: &TastyTradeConfig,
        code: impl Into<AuthorizationCode>,
    ) -> TastyResult<Self> {
        if config.client_id.trim().is_empty()
            || config.client_secret.is_blank()
            || config.redirect_uri.trim().is_empty()
        {
            return Err(crate::TastyTradeError::ConfigError(
                "the authorization-code grant needs TASTYTRADE_CLIENT_ID, \
                 TASTYTRADE_CLIENT_SECRET and TASTYTRADE_REDIRECT_URI, and the redirect URI must \
                 be the same one the authorization request used"
                    .to_string(),
            ));
        }

        Self::establish(
            config,
            authorization_code_grant(
                code.into(),
                config.client_id.clone(),
                config.client_secret.clone(),
                config.redirect_uri.clone(),
            ),
        )
        .await
    }

    /// Builds the HTTP client and performs the first token exchange.
    async fn establish(
        config: &TastyTradeConfig,
        grant: crate::oauth::OAuthGrant,
    ) -> TastyResult<Self> {
        let client = ClientBuilder::new()
            .default_headers(default_headers())
            .build()
            .map_err(|e| {
                crate::TastyTradeError::Connection(format!("could not build the HTTP client: {e}"))
            })?;

        let session = OAuthSession::establish(
            client.clone(),
            &config.base_url,
            config.environment(),
            grant,
        )
        .await?;

        // Deliberately not the customer, the token or the grant: the caller
        // already knows which credentials it supplied, and the environment is
        // the part worth confirming.
        debug!("Authenticated against {}", config.environment());

        Ok(Self {
            client,
            session,
            config: config.clone(),
        })
    }

    /// The OAuth session behind this client.
    ///
    /// Exposes token lifetime and the refresh token; see [`OAuthSession`].
    pub fn session(&self) -> &Arc<OAuthSession> {
        &self.session
    }

    /// The refresh token this client would renew with, if it has one.
    ///
    /// An application that authenticated with an authorization code must store
    /// this: it is what lets the next process start with [`TastyTrade::connect`]
    /// instead of another trip through the authorization page.
    pub async fn refresh_token(&self) -> Option<RefreshToken> {
        self.session.refresh_token().await
    }

    /// A live access token, refreshing first if the current one is about to
    /// expire.
    ///
    /// Needed by anything that authenticates outside the REST path — the
    /// account websocket puts the same `Bearer `-prefixed value in its
    /// `auth-token` field.
    ///
    /// # Errors
    ///
    /// As [`OAuthSession::access_token`].
    pub async fn access_token(&self) -> TastyResult<AccessToken> {
        self.session.ensure_same_deployment(&self.config.base_url)?;
        self.session.access_token().await
    }

    /// An `Authorization` header carrying a live access token.
    ///
    /// Built here rather than at each call site so no verb can forget the
    /// refresh or the `Bearer ` prefix.
    async fn authorization(&self) -> TastyResult<HeaderValue> {
        let token = self.access_token().await?;
        // A token with bytes a header cannot carry is a venue response this
        // crate cannot use. The error says so without quoting the token.
        HeaderValue::from_str(&token.bearer()).map_err(|_| {
            crate::TastyTradeError::Auth(
                "the access token returned by the venue is not a valid header value".to_string(),
            )
        })
    }

    /// Joins the base URL and a path, after checking the session belongs here.
    fn request_url(&self, path: &str) -> TastyResult<String> {
        self.session.ensure_same_deployment(&self.config.base_url)?;
        endpoint_url(&self.config.base_url, path)
    }

    /// Performs a GET with query parameters and decodes the response.
    ///
    /// `T` is the payload inside the envelope; `R` is what you want back, so
    /// asking for a `Paginated<T>` from an endpoint that does not paginate is
    /// an error rather than a panic.
    ///
    /// # Errors
    ///
    /// A non-2xx response becomes [`crate::TastyTradeError::Request`] carrying
    /// a redacted endpoint, the environment and the status. Response bodies
    /// never reach the error or the logs.
    pub async fn get_with_query<T, R, U>(&self, url: U, query: &[(&str, &str)]) -> TastyResult<R>
    where
        T: DeserializeOwned + Serialize + std::fmt::Debug,
        R: FromTastyResponse<T>,
        U: AsRef<str>,
    {
        let full_url = self.request_url(url.as_ref())?;
        let query_string = query
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let full_request = if query_string.is_empty() {
            full_url.clone()
        } else {
            format!("{}?{}", full_url, query_string)
        };
        // Errors travel wherever the caller sends them, and every account-scoped
        // path carries the account number in the URL, so error context uses the
        // redacted form. The full URL stays at DEBUG.
        let report = RequestReport::new(
            "GET",
            redact_account_path(&full_request),
            self.config.environment(),
        );
        // Refreshed before the request rather than retried after a 401. The
        // token is minted for fifteen minutes and every verb goes through
        // here, so a caller never has to think about expiry.
        let authorization = self.authorization().await?;
        // A timeout or a refused connection is the failure most worth retrying,
        // and it used to exit through From<reqwest::Error> before any context
        // existed, so the caller lost the method, the endpoint and the
        // environment on exactly those.
        let response: reqwest::Response = if query.is_empty() {
            self.client
                .get(&full_url)
                .header(header::AUTHORIZATION, authorization)
                .send()
                .await
                .map_err(|e| transport_failure(&report, e))?
        } else {
            let mut url_with_query = reqwest::Url::parse(&full_url).map_err(|e| {
                crate::TastyTradeError::Unknown(format!("Failed to parse URL: {}", e))
            })?;
            {
                let mut query_pairs = url_with_query.query_pairs_mut();
                for (k, v) in query {
                    query_pairs.append_pair(k, v);
                }
            }
            self.client
                .get(url_with_query)
                .header(header::AUTHORIZATION, authorization)
                .send()
                .await
                .map_err(|e| transport_failure(&report, e))?
        };

        decode_response::<T, R>(&report, response).await
    }

    /// Performs a GET with no query parameters.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::get_with_query`].
    pub async fn get<T: DeserializeOwned + Serialize + std::fmt::Debug, U: AsRef<str>>(
        &self,
        url: U,
    ) -> TastyResult<T> {
        self.get_with_query(url, &[]).await
    }

    /// Performs a POST with a JSON payload.
    ///
    /// **This can mutate account state**, including placing an order. Prefer
    /// [`crate::accounts::Account::review_order`] and
    /// [`crate::accounts::Account::place_reviewed_order`] for anything that
    /// trades.
    ///
    /// # Errors
    ///
    /// Fails without contacting the venue when the payload cannot be
    /// serialized. Otherwise as [`TastyTrade::get_with_query`]: a non-2xx
    /// response becomes [`crate::TastyTradeError::Request`] carrying a redacted
    /// endpoint, the environment and the status, and no response body reaches
    /// the error or the logs.
    ///
    /// A `401` here is **not** retried with a fresh token. The token is
    /// renewed before the request goes out, so a refusal after that is the
    /// venue's answer — and a POST that may have placed an order is the one
    /// request this crate will never replay on its own.
    pub async fn post<R, P, U>(&self, url: U, payload: P) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        P: Serialize,
        U: AsRef<str>,
    {
        let full_url = self.request_url(url.as_ref())?;
        let report = RequestReport::new(
            "POST",
            redact_account_path(&full_url),
            self.config.environment(),
        );

        let authorization = self.authorization().await?;
        let response = self
            .client
            .post(&full_url)
            .header(header::AUTHORIZATION, authorization)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(serde_json::to_string(&payload)?)
            .send()
            .await
            .map_err(|e| transport_failure(&report, e))?;

        decode_response::<R, R>(&report, response).await
    }

    /// Performs a PUT with a JSON payload.
    ///
    /// **This can mutate account state**, including replacing a working order.
    /// Prefer [`crate::accounts::Account::review_amendment`] and
    /// [`crate::accounts::Account::place_reviewed_amendment`].
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::post`], including that a `401` is not retried with a
    /// fresh token.
    pub async fn put<R, P, U>(&self, url: U, payload: P) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        P: Serialize,
        U: AsRef<str>,
    {
        self.mutate("PUT", url, payload, reqwest::Method::PUT).await
    }

    /// Performs a PATCH with a JSON payload.
    ///
    /// **This can mutate account state**, including editing a working order.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::put`].
    pub async fn patch<R, P, U>(&self, url: U, payload: P) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        P: Serialize,
        U: AsRef<str>,
    {
        self.mutate("PATCH", url, payload, reqwest::Method::PATCH)
            .await
    }

    /// The body of `put` and `patch`, which differ only in the verb.
    ///
    /// Shared rather than copied so the two cannot drift apart on the parts
    /// that matter: the deployment check, the pre-request refresh, the redacted
    /// operation in the error, and the single place the status is inspected.
    async fn mutate<R, P, U>(
        &self,
        method: &'static str,
        url: U,
        payload: P,
        verb: reqwest::Method,
    ) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        P: Serialize,
        U: AsRef<str>,
    {
        let full_url = self.request_url(url.as_ref())?;
        let report = RequestReport::new(
            method,
            redact_account_path(&full_url),
            self.config.environment(),
        );

        let authorization = self.authorization().await?;
        let response = self
            .client
            .request(verb, &full_url)
            .header(header::AUTHORIZATION, authorization)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(serde_json::to_string(&payload)?)
            .send()
            .await
            .map_err(|e| transport_failure(&report, e))?;

        decode_response::<R, R>(&report, response).await
    }

    /// Performs a DELETE.
    ///
    /// **This can mutate account state**, including cancelling an order.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::get_with_query`]. A `404` here usually means the target
    /// is already gone, which the status in the error tells apart from a
    /// request that never arrived.
    pub async fn delete<R, U>(&self, url: U) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        U: AsRef<str>,
    {
        let full_url = self.request_url(url.as_ref())?;
        let report = RequestReport::new(
            "DELETE",
            redact_account_path(&full_url),
            self.config.environment(),
        );

        let authorization = self.authorization().await?;
        let response = self
            .client
            .delete(&full_url)
            .header(header::AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(|e| transport_failure(&report, e))?;

        decode_response::<R, R>(&report, response).await
    }

    /// `DELETE` an endpoint that answers `204 No Content`.
    ///
    /// Separate from [`TastyTrade::delete`] because the difference is not
    /// cosmetic: asking the generic verb for a payload makes it try to decode
    /// an empty body, so the mutation succeeds and the caller is handed a
    /// decode error for it. The only honest reading of that error is "it may
    /// or may not have happened", which is the worst answer available about a
    /// state change.
    ///
    /// A failure status still goes through the shared decoder, so the broker's
    /// error document is reported the same way it is everywhere else.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub(crate) async fn delete_no_content<U: AsRef<str>>(&self, url: U) -> TastyResult<()> {
        let full_url = self.request_url(url.as_ref())?;
        let report = RequestReport::new(
            "DELETE",
            redact_account_path(&full_url),
            self.config.environment(),
        );

        let authorization = self.authorization().await?;
        let response = self
            .client
            .delete(&full_url)
            .header(header::AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(|e| transport_failure(&report, e))?;

        if response.status().is_success() {
            debug!(
                "{} {} -> {} in {:?}",
                report.method,
                report.operation,
                response.status().as_u16(),
                report.started.elapsed()
            );
            return Ok(());
        }

        // Reuses the failure half of the shared decoder, which sanitises the
        // broker's error document and never logs the body.
        decode_response::<serde_json::Value, serde_json::Value>(&report, response)
            .await
            .map(|_| ())
    }

    /// Every account on this session.
    ///
    /// # Errors
    ///
    /// Fails when the listing arrives but nothing in it can be decoded, which
    /// is a defect in this crate rather than an empty account list. An account
    /// list that is genuinely empty is `Ok`.
    pub async fn accounts(&self) -> TastyResult<Vec<Account<'_>>> {
        let resp: Items<AccountInner> = self.get("/customers/me/accounts").await?;
        Ok(resp
            .into_items()?
            .into_iter()
            .map(|inner| Account { inner, tasty: self })
            .collect())
    }

    /// One account by number, or `None` when this session cannot see it.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::accounts`], which this uses.
    pub async fn account(
        &self,
        account_number: impl Into<AccountNumber>,
    ) -> TastyResult<Option<Account<'_>>> {
        // One request for one account, rather than the whole listing filtered
        // here. The old version could not tell "this session cannot see that
        // account" from "a *sibling* account failed to deserialize, so
        // `Items<T>` skipped it and the one you asked for vanished with it" —
        // which is exactly what the `is-test-drive` bug looked like from
        // outside. Now `Ok(None)` means the venue said 404.
        self.account_by_number(account_number).await
    }

    /// One account by number, from its own endpoint.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error. A `404` is **not** an error: it is
    /// `Ok(None)`, meaning this session cannot see that account. Every other
    /// status is reported.
    pub async fn account_by_number(
        &self,
        account_number: impl Into<AccountNumber>,
    ) -> TastyResult<Option<Account<'_>>> {
        let account_number = account_number.into();
        let path = format!(
            "/customers/me/accounts/{}",
            encode_path_segment(&account_number.0)
        );

        match self.get::<AccountDetails, _>(&path).await {
            Ok(account) => Ok(Some(Account {
                inner: AccountInner {
                    account,
                    // The single-account endpoint answers with the account
                    // itself rather than the listing's authority decorator, so
                    // there is no authority level to report. Saying so is
                    // better than inventing "owner".
                    authority_level: String::new(),
                },
                tasty: self,
            })),
            Err(crate::TastyTradeError::Request { context, .. }) if context.status == Some(404) => {
                Ok(None)
            }
            Err(other) => Err(other),
        }
    }

    /// The venue's public margin configuration.
    ///
    /// Public and read-only, but it goes through the authenticated client like
    /// everything else: one transport, one error shape, one place the status is
    /// checked.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn margin_requirements_configuration(&self) -> TastyResult<MarginConfiguration> {
        self.get("/margin-requirements-public-configuration").await
    }

    /// One snapshot of prices for up to a hundred symbols.
    ///
    /// The REST alternative to opening a DXLink channel and waiting: a
    /// portfolio mark, a screener pass, a pre-trade sanity check. Every price
    /// is `Decimal` — the `f64` exemption is for the streaming types, where the
    /// feed imposes it, and these are not those types.
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the request asks for more
    /// than [`crate::prelude::MAX_MARKET_DATA_SYMBOLS`] symbols across all
    /// types, or for none at all. Fails when snapshots arrive but none can be
    /// decoded; a genuinely empty result is `Ok`.
    pub async fn market_data_by_type(
        &self,
        request: &MarketDataRequest,
    ) -> TastyResult<Vec<MarketDataSnapshot>> {
        request.validate()?;

        let query = request.to_query();
        let resp: Items<MarketDataSnapshot> = self
            .get_with_query("/market-data/by-type", &query.pairs())
            .await?;
        resp.into_items()
    }

    /// Volatility and liquidity for several underlyings.
    ///
    /// `symbols` is **comma-joined into one parameter**, which is how the venue
    /// documents it and unlike the repeated keys the instrument listings use.
    ///
    /// **Live only**: the venue's sandbox page lists Market Metrics as
    /// unavailable in certification.
    ///
    /// # Errors
    ///
    /// Fails when metrics arrive but none can be decoded; a genuinely empty
    /// result is `Ok`.
    pub async fn market_metrics(
        &self,
        symbols: &[impl AsRef<str>],
    ) -> TastyResult<Vec<MarketMetric>> {
        let query = symbols_query(symbols)?;
        let resp: Items<MarketMetric> = self
            .get_with_query("/market-metrics", &query.pairs())
            .await?;
        resp.into_items()
    }

    /// An underlying's dividend history.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::market_metrics`].
    pub async fn historic_dividends(&self, symbol: &str) -> TastyResult<Vec<DividendReport>> {
        let resp: Items<DividendReport> = self
            .get(format!(
                "/market-metrics/historic-corporate-events/dividends/{}",
                encode_path_segment(symbol)
            ))
            .await?;
        resp.into_items()
    }

    /// An underlying's earnings history over a range.
    ///
    /// [`EarningsRange`] carries the start date the venue requires, so it
    /// cannot be omitted.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::market_metrics`].
    pub async fn historic_earnings(
        &self,
        symbol: &str,
        range: &EarningsRange,
    ) -> TastyResult<Vec<EarningsReport>> {
        let query = range.to_query();
        let resp: Items<EarningsReport> = self
            .get_with_query(
                format!(
                    "/market-metrics/historic-corporate-events/earnings-reports/{}",
                    encode_path_segment(symbol)
                ),
                &query.pairs(),
            )
            .await?;
        resp.into_items()
    }

    /// tastytrade's own curated watchlists.
    ///
    /// `counts_only` asks the venue for the lists without their entries, which
    /// is what the streaming half publishes alongside.
    ///
    /// # Errors
    ///
    /// Fails when lists arrive but none can be decoded.
    pub async fn public_watchlists(&self, counts_only: bool) -> TastyResult<Vec<Watchlist>> {
        let mut query = QueryBuilder::new();
        // Sent only when asked for, so the venue's own default survives.
        if counts_only {
            query.push_flag("counts-only", Some(true));
        }

        let resp: Items<Watchlist> = self
            .get_with_query("/public-watchlists", &query.pairs())
            .await?;
        resp.into_items()
    }

    /// The curated watchlists without their entries.
    ///
    /// The venue does not publish a schema for the counts-only response, so
    /// what comes back decodes into [`Watchlist`] with whatever fields arrived
    /// — the entry list simply ends up empty.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::public_watchlists`].
    pub async fn public_watchlist_counts(&self) -> TastyResult<Vec<Watchlist>> {
        self.public_watchlists(true).await
    }

    /// One curated watchlist by name.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404`.
    pub async fn public_watchlist(&self, name: &str) -> TastyResult<Watchlist> {
        self.get(format!("/public-watchlists/{}", encode_path_segment(name)))
            .await
    }

    /// This user's own watchlists.
    ///
    /// # Errors
    ///
    /// Fails when lists arrive but none can be decoded.
    pub async fn watchlists(&self) -> TastyResult<Vec<Watchlist>> {
        let resp: Items<Watchlist> = self.get("/watchlists").await?;
        resp.into_items()
    }

    /// One of this user's watchlists by name.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404`.
    pub async fn watchlist(&self, name: &str) -> TastyResult<Watchlist> {
        self.get(format!("/watchlists/{}", encode_path_segment(name)))
            .await
    }

    /// Creates a watchlist.
    ///
    /// **Creates user data.**
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the name is blank or an
    /// entry has a blank symbol. Propagates the venue's error otherwise,
    /// including a refusal to create a list that already exists.
    pub async fn create_watchlist(&self, watchlist: &NewWatchlist) -> TastyResult<Watchlist> {
        watchlist.validate()?;

        self.post("/watchlists", watchlist).await
    }

    /// **Replaces every property** of a watchlist.
    ///
    /// This is not an append and not a merge. The entries in `watchlist` are
    /// the entries that survive: anything on the list and not in this request
    /// is gone. To add a symbol, read the list, push onto its entries, and send
    /// the whole thing back.
    ///
    /// **Destroys user data** when the entries are narrower than what is there.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::create_watchlist`].
    pub async fn replace_watchlist(
        &self,
        name: &str,
        watchlist: &NewWatchlist,
    ) -> TastyResult<Watchlist> {
        watchlist.validate()?;

        self.put(
            format!("/watchlists/{}", encode_path_segment(name)),
            watchlist,
        )
        .await
    }

    /// Deletes a watchlist.
    ///
    /// **Destroys user data, irreversibly.** It is its own method taking its
    /// own argument, so it cannot be reached from a listing or a read by
    /// accident.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404` for a list that is
    /// already gone.
    pub async fn delete_watchlist(&self, name: &str) -> TastyResult<Watchlist> {
        self.delete(format!("/watchlists/{}", encode_path_segment(name)))
            .await
    }

    /// Every pairs watchlist.
    ///
    /// # Errors
    ///
    /// Fails when lists arrive but none can be decoded.
    pub async fn pairs_watchlists(&self) -> TastyResult<Vec<PairsWatchlist>> {
        let resp: Items<PairsWatchlist> = self.get("/pairs-watchlists").await?;
        resp.into_items()
    }

    /// One pairs watchlist by name.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404`.
    pub async fn pairs_watchlist(&self, name: &str) -> TastyResult<PairsWatchlist> {
        self.get(format!("/pairs-watchlists/{}", encode_path_segment(name)))
            .await
    }

    /// Every quote alert this **user** has set.
    ///
    /// Alerts are per user, not per account, which is why this hangs off the
    /// client rather than off [`crate::accounts::Account`] — putting it there
    /// would imply a scoping the venue does not have.
    ///
    /// # Errors
    ///
    /// Fails when alerts arrive but none can be decoded; a user with no alerts
    /// is `Ok` with an empty vector.
    pub async fn quote_alerts(&self) -> TastyResult<Vec<QuoteAlert>> {
        let resp: Items<QuoteAlert> = self.get("/quote-alerts").await?;
        resp.into_items()
    }

    /// Sets a quote alert.
    ///
    /// The alert fires over the **account websocket**, to a caller subscribed
    /// with [`crate::prelude::SubRequestAction::QuoteAlertsSubscribe`] — and it
    /// arrives as the same [`QuoteAlert`] type this returns, so the two halves
    /// cannot drift apart.
    ///
    /// # Errors
    ///
    /// Fails **before sending anything** with
    /// [`crate::TastyTradeError::Precondition`] when the symbol or threshold is
    /// blank, or the threshold is zero. Propagates the venue's error otherwise.
    pub async fn create_quote_alert(&self, alert: &NewQuoteAlert) -> TastyResult<QuoteAlert> {
        alert.validate()?;

        self.post("/quote-alerts", alert).await
    }

    /// Cancels a quote alert.
    ///
    /// **Mutates user state.**
    ///
    /// Returns nothing, because the venue returns nothing: the published
    /// contract answers `204 No Content`. Asking for a [`QuoteAlert`] back
    /// made the call cancel the alert and then fail decoding the empty body,
    /// so a successful cancellation was reported as an error.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404` for an alert that is
    /// already gone.
    pub async fn cancel_quote_alert(&self, alert_external_id: &str) -> TastyResult<()> {
        self.delete_no_content(format!(
            "/quote-alerts/{}",
            encode_path_segment(alert_external_id)
        ))
        .await
    }

    /// One page of SPAN risk rows for a date and exchange.
    ///
    /// Both parameters are required by the venue, so both are arguments rather
    /// than optional fields — a required query parameter should be impossible
    /// to omit, not a runtime `400`. `exchange` is a closed set for the same
    /// reason: the published contract admits two values, and a typo or a blank
    /// string should not survive to an authenticated round trip.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint answers without a pagination block, and
    /// propagates the venue's error otherwise.
    pub async fn span_rows(
        &self,
        date: NaiveDate,
        exchange: SpanExchange,
        page: &PageRequest,
    ) -> TastyResult<Paginated<SpanRow>> {
        let mut query = QueryBuilder::new();
        query.push("date", date);
        query.push("exchange", exchange.as_wire());
        page.write_into(&mut query);

        self.get_with_query::<Items<SpanRow>, _, _>("/span/rows", &query.pairs())
            .await
    }

    /// One page of order history across several accounts.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint answers without a pagination block, and when
    /// orders arrive but none can be decoded.
    pub async fn customer_orders(
        &self,
        filter: &CustomerOrderFilter,
    ) -> TastyResult<Paginated<LiveOrderRecord>> {
        let query = filter.to_query();
        self.get_with_query::<Items<LiveOrderRecord>, _, _>("/customers/me/orders", &query.pairs())
            .await
    }

    /// One page of working orders across several accounts.
    ///
    /// # Errors
    ///
    /// As [`TastyTrade::customer_orders`].
    pub async fn customer_live_orders(
        &self,
        filter: &CustomerLiveOrderFilter,
    ) -> TastyResult<Paginated<LiveOrderRecord>> {
        let query = filter.to_query();
        self.get_with_query::<Items<LiveOrderRecord>, _, _>(
            "/customers/me/orders/live",
            &query.pairs(),
        )
        .await
    }

    /// The full customer resource for this session.
    ///
    /// **This is personal data** — names, addresses, tax identifiers, birth
    /// dates. [`Customer`] renders as a field count and nothing else; reading a
    /// value means naming the field.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn customer(&self) -> TastyResult<Customer> {
        self.customer_by_id("me").await
    }

    /// The full customer resource for one customer id.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error, including a `404` for a customer this
    /// session cannot see. Use [`TastyTrade::find_customer`] when a missing
    /// customer is an ordinary answer rather than a failure.
    pub async fn customer_by_id(&self, customer_id: &str) -> TastyResult<Customer> {
        self.get(format!("/customers/{}", encode_path_segment(customer_id)))
            .await
    }

    /// The customer resource, or nothing, without a `404`.
    ///
    /// Sends the venue's documented `allow-missing`, which suppresses the
    /// `404`. A response that carries no `id` is treated as no customer: the
    /// venue's own way of saying "not found" once it has been told not to
    /// raise, and a customer with no identifier is not one this crate can hand
    /// back as if it were real.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error. A missing customer is `Ok(None)`.
    pub async fn find_customer(&self, customer_id: &str) -> TastyResult<Option<Customer>> {
        let customer: Option<Customer> = self
            .get_with_query::<Option<Customer>, _, _>(
                format!("/customers/{}", encode_path_segment(customer_id)),
                &[("allow-missing", "true")],
            )
            .await?;

        Ok(customer.filter(|customer| customer.id.is_some()))
    }

    /// Opens a DXLink market-data streamer.
    ///
    /// # Errors
    ///
    /// Fails when the streamer token cannot be obtained or the DXLink
    /// connection cannot be established.
    pub async fn create_quote_streamer(&self) -> TastyResult<QuoteStreamer> {
        QuoteStreamer::connect(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every spelling the WHATWG URL Standard folds into a dot segment, and
    /// the near misses that must still go out. The table is the point: the
    /// standard's list is longer than "`.` and `..`", and the entries that
    /// mix escaped and literal dots are the ones a hand-rolled check misses.
    #[test]
    fn a_dot_only_segment_is_refused_and_a_dot_inside_one_is_not() {
        for marker in [".", "..", "%2e", "%2E", "%2e%2e", "%2E%2E", ".%2e", "%2E."] {
            let path = format!("/instruments/equities/{marker}");
            assert!(
                matches!(
                    endpoint_url("https://api.example.com", &path),
                    Err(crate::TastyTradeError::Precondition(_))
                ),
                "{marker:?} was accepted, and URL resolution would have removed it"
            );
        }

        for value in ["BRK.B", ".%2FESZ4", "...", "%252E", "a.", ".a", "%2ex"] {
            let path = format!("/instruments/equities/{value}");
            assert_eq!(
                endpoint_url("https://api.example.com", &path).expect("an ordinary value"),
                format!("https://api.example.com{path}")
            );
        }
    }

    /// The guard reads the path, so it must stop reading where the path does.
    /// A dot segment inside a query value is data the venue parses, not a
    /// marker URL resolution acts on.
    #[test]
    fn a_dot_segment_in_the_query_string_is_not_a_path_segment() {
        let path = "/accounts/5WX12345/transactions?sort=..&start-date=2026-01-01";
        let url = endpoint_url("https://api.example.com", path).expect("a query value");
        assert!(url.ends_with(path), "{url}");
    }

    /// The refusal explains itself without naming the account it was reaching
    /// for. `Precondition` messages go to the same places every other error
    /// does, so the redaction rule is not relaxed just because nothing was
    /// sent.
    #[test]
    fn the_refusal_carries_no_account_number() {
        let error = endpoint_url("https://api.example.com", "/accounts/5WX12345/orders/..")
            .expect_err("a dot segment must be refused");
        let rendered = error.to_string();

        assert!(!rendered.contains("5WX12345"), "{rendered}");
        assert!(rendered.contains("{account}"), "{rendered}");
    }

    #[test]
    fn redacts_the_account_number_from_an_account_scoped_path() {
        assert_eq!(
            redact_account_path("https://api.tastyworks.com/accounts/5WX12345/balances"),
            "https://api.tastyworks.com/accounts/{account}/balances"
        );
    }

    #[test]
    fn redacts_the_account_number_ahead_of_a_query_string() {
        let redacted = redact_account_path(
            "https://api.tastyworks.com/accounts/5WX12345/balance-snapshots?start-date=2026-01-01",
        );

        assert!(!redacted.contains("5WX12345"), "{redacted}");
        assert!(redacted.contains("start-date=2026-01-01"), "{redacted}");
    }

    #[test]
    fn redacts_every_segment_that_follows_accounts() {
        assert_eq!(
            redact_account_path("https://api.tastyworks.com/accounts/5WX12345/orders/187"),
            "https://api.tastyworks.com/accounts/{account}/orders/187"
        );
    }

    #[test]
    fn leaves_paths_without_an_account_segment_alone() {
        for url in [
            "https://api.tastyworks.com/customers/me/accounts",
            "https://api.tastyworks.com/instruments/equities/AAPL",
            "https://api.tastyworks.com/option-chains/SPY/nested",
        ] {
            assert_eq!(redact_account_path(url), url);
        }
    }

    /// A customer identifier is the account number's equal, and it reaches the
    /// same places: `RequestReport.operation`, every DEBUG line about the
    /// request, and the `Display` of every error the request produces. It was
    /// passing through because the redaction only knew about `/accounts/`.
    #[test]
    fn redacts_the_customer_identifier() {
        assert_eq!(
            redact_account_path("https://api.tastyworks.com/customers/78a1f0c2-4d31"),
            "https://api.tastyworks.com/customers/{customer}"
        );
        // Both identifiers in one path, which is what the account endpoints
        // under a named customer look like.
        assert_eq!(
            redact_account_path("https://api.tastyworks.com/customers/78a1f0c2/accounts/5WX12345"),
            "https://api.tastyworks.com/customers/{customer}/accounts/{account}"
        );
        // And with a query string, which the paginated listings carry.
        let redacted =
            redact_account_path("https://api.tastyworks.com/customers/78a1f0c2?per-page=100");
        assert!(!redacted.contains("78a1f0c2"), "{redacted}");
        assert!(redacted.contains("per-page=100"), "{redacted}");
    }

    /// The customer order endpoints take the identifiers as query parameters
    /// rather than path segments, so redacting the path alone left them in
    /// `RequestReport.operation` and from there in every error the request
    /// produced. The parameter names survive: an error that cannot say which
    /// request failed is worse than one that says too much.
    #[test]
    fn redacts_an_account_number_carried_in_the_query() {
        let redacted = redact_account_path(
            "https://api.tastyworks.com/orders?account-numbers[]=5WX12345&\
             account-numbers[]=5WX00002&per-page=50",
        );

        assert!(!redacted.contains("5WX12345"), "{redacted}");
        assert!(!redacted.contains("5WX00002"), "{redacted}");
        assert_eq!(redacted.matches("{account}").count(), 2, "{redacted}");
        // The names and the unrelated parameters are context worth keeping.
        assert!(redacted.contains("account-numbers[]="), "{redacted}");
        assert!(redacted.contains("per-page=50"), "{redacted}");

        // The HTTP client percent-encodes the brackets, which is the form the
        // redaction actually sees on a real request.
        let redacted =
            redact_account_path("https://api.tastyworks.com/orders?account-numbers%5B%5D=5WX12345");
        assert!(!redacted.contains("5WX12345"), "{redacted}");

        // The singular spelling and the qualified one the trading status uses.
        let redacted = redact_account_path(
            "https://api.tastyworks.com/x?account-number=5WX12345&\
             clearing-account-number=99887&sort=Desc",
        );
        assert!(!redacted.contains("5WX12345"), "{redacted}");
        assert!(!redacted.contains("99887"), "{redacted}");
        assert!(redacted.contains("sort=Desc"), "{redacted}");
    }

    /// `me` is a literal the venue defines rather than an identifier. It is the
    /// form nearly every request uses, and redacting it would cost the whole
    /// path its readability while hiding nothing.
    #[test]
    fn leaves_the_me_alias_readable() {
        for url in [
            "https://api.tastyworks.com/customers/me",
            "https://api.tastyworks.com/customers/me/accounts",
        ] {
            assert_eq!(redact_account_path(url), url);
        }
        // The account number after it is still redacted.
        assert_eq!(
            redact_account_path("https://api.tastyworks.com/customers/me/accounts/5WX12345"),
            "https://api.tastyworks.com/customers/me/accounts/{account}"
        );
    }
}
