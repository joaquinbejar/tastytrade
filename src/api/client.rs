use std::fmt::Display;

use crate::accounts::{Account, AccountInner, AccountNumber};
use crate::api::base::Items;
use crate::api::base::Paginated;
use crate::api::base::Response;
use crate::api::base::TastyApiResponse;
use crate::api::base::TastyResult;
use crate::error::{ApiError, InnerApiError};
use crate::streaming::quote_streamer::QuoteStreamer;
use crate::types::login::{LoginCredentials, LoginResponse};
use crate::utils::config::TastyTradeConfig;
use reqwest::ClientBuilder;
use reqwest::header;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::debug;

/// An authenticated tastytrade session.
///
/// Holds the session token and the configuration it was built from, which
/// includes the password, so its `Debug` redacts both. Cloning is cheap and
/// shares the underlying HTTP client.
#[derive(Clone)]
pub struct TastyTrade {
    pub(crate) client: reqwest::Client,
    pub(crate) session_token: String,
    pub(crate) config: TastyTradeConfig,
}

impl std::fmt::Debug for TastyTrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TastyTrade")
            .field("session_token", &"***")
            .field("config", &self.config)
            .finish()
    }
}

impl Display for TastyTrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TastyTrade")
    }
}

/// Replaces the account number in an account-scoped URL with a placeholder.
///
/// Every `/accounts/{number}/…` request embeds the identifier in its path. That
/// URL is useful context in an error, the identifier is not: an error value is
/// logged, reported, or shown wherever the caller decides, so it must not carry
/// something the caller never chose to handle.
fn redact_account_path(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let mut redact_next = false;

    for (index, segment) in url.split('/').enumerate() {
        if index > 0 {
            out.push('/');
        }
        if redact_next && !segment.is_empty() {
            out.push_str("{account}");
            redact_next = false;
        } else {
            out.push_str(segment);
            redact_next = segment == "accounts";
        }
    }

    out
}

/// Wraps a transport failure in the same sanitised shape as a venue failure.
///
/// The reqwest error itself is kept out of the value: its `Display` renders
/// the URL it was trying to reach, account number included.
fn transport_failure(
    context: crate::error::RequestContext,
    error: reqwest::Error,
) -> crate::TastyTradeError {
    debug!("transport failure on {}: {}", context.operation, error);
    crate::TastyTradeError::Request { context, api: None }
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
    /// Logs in using configuration read from the environment.
    ///
    /// # Errors
    ///
    /// Fails without a network request when the credentials are missing, and
    /// with the venue's own error when they are rejected. Certification is the
    /// default environment; see [`crate::utils::config::TastyTradeConfig`].
    pub async fn default() -> TastyResult<Self> {
        let config = TastyTradeConfig::default();
        Self::login(&config).await
    }

    /// Logs in with an explicit configuration.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` without contacting the venue when the username or
    /// password is missing or blank. The error names the variables to set and
    /// never their values.
    pub async fn login(config: &TastyTradeConfig) -> TastyResult<Self> {
        // Fail here rather than posting an empty credential pair to the venue.
        // The message names the variables, never their values.
        if !config.has_valid_credentials() {
            return Err(crate::TastyTradeError::ConfigError(
                "Missing tastytrade credentials: set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD, \
                 or load a configuration file that provides both"
                    .to_string(),
            ));
        }

        let creds = Self::do_login_request(
            &config.username,
            &config.password,
            config.remember_me,
            &config.base_url,
        )
        .await?;

        // Deliberately not the email: it identifies the account holder, and
        // the caller already knows which credentials it supplied.
        debug!("Login successful");
        let client = Self::create_client(&creds)?;

        Ok(Self {
            client,
            session_token: creds.session_token,
            config: config.clone(),
        })
    }

    fn create_client(creds: &LoginResponse) -> TastyResult<reqwest::Client> {
        let mut headers = HeaderMap::new();

        // A session token with bytes a header cannot carry is a venue
        // response this crate cannot use. The error says so without quoting
        // the token.
        let token = HeaderValue::from_str(&creds.session_token).map_err(|_| {
            crate::TastyTradeError::Auth(
                "the session token returned by the venue is not a valid header value".to_string(),
            )
        })?;
        headers.insert(header::AUTHORIZATION, token);
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(header::USER_AGENT, HeaderValue::from_static("tastytrade"));

        ClientBuilder::new()
            .default_headers(headers)
            .build()
            .map_err(|e| {
                crate::TastyTradeError::Connection(format!("could not build the HTTP client: {e}"))
            })
    }

    async fn do_login_request(
        login: &str,
        password: &str,
        remember_me: bool,
        base_url: &str,
    ) -> TastyResult<LoginResponse> {
        let client = reqwest::Client::default();

        let resp = client
            .post(format!("{base_url}/sessions"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, "tastytrade")
            .json(&LoginCredentials {
                login: login.to_string(),
                password: password.to_string(),
                remember_me,
            })
            .send()
            .await?;
        let json = resp
            //.inspect_json::<TastyApiResponse<LoginResponse>, TastyError>(|text| println!("{text}"))
            .json()
            .await?;
        let response = match json {
            TastyApiResponse::Success(s) => Ok(s),
            TastyApiResponse::Error { error } => Err(error),
        }?
        .data;

        Ok(response)
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
        let full_url = format!("{}{}", self.config.base_url, url.as_ref());
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
        let request_info = redact_account_path(&full_request);
        let started = std::time::Instant::now();

        // A timeout or a refused connection is the failure most worth
        // retrying, and it used to exit through From<reqwest::Error> before any
        // context existed, so the caller lost the method, the endpoint and the
        // environment on exactly those. Same shape as every other failure.
        let transport_context = |status| crate::error::RequestContext {
            method: "GET",
            operation: request_info.clone(),
            environment: self.config.environment(),
            status,
        };

        let response: reqwest::Response = if query.is_empty() {
            self.client
                .get(&full_url)
                .send()
                .await
                .map_err(|e| transport_failure(transport_context(None), e))?
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
                .send()
                .await
                .map_err(|e| transport_failure(transport_context(None), e))?
        };

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            // The body is not logged at any level. An error response comes
            // from an endpoint this code does not control, /sessions included,
            // so it can echo a credential; the secrecy invariant has no DEBUG
            // exemption. What is safe is the shape of it.
            let parsed = serde_json::from_str::<TastyApiResponse<serde_json::Value>>(&body);
            debug!(
                "GET {} -> {} ({} bytes, {})",
                request_info,
                status.as_u16(),
                body.len(),
                match &parsed {
                    Ok(TastyApiResponse::Error { .. }) => "broker error document",
                    Ok(TastyApiResponse::Success(_)) => "success envelope on a failure status",
                    Err(_) => "unrecognised body",
                }
            );

            // A tastytrade error document is the useful case: it carries the
            // broker's own code and message, which is what a caller can act
            // on. Anything else degrades to the status and the endpoint.
            // One shape for every failed request: sanitised context plus the
            // broker's own document when it sent one. The body never travels.
            return Err(crate::TastyTradeError::Request {
                context: crate::error::RequestContext {
                    method: "GET",
                    operation: request_info,
                    environment: self.config.environment(),
                    status: Some(status.as_u16()),
                },
                api: match serde_json::from_str::<TastyApiResponse<serde_json::Value>>(&body) {
                    Ok(TastyApiResponse::Error { error }) => Some(sanitize_api_error(error)),
                    _ => None,
                },
            });
        }

        let text = response.text().await?;
        // A successful body is account numbers, balances, positions and order
        // contents. Nothing about it belongs in a consumer's logs, so this
        // reports the shape of the exchange and not its contents.
        debug!(
            "GET {} -> {} ({} bytes in {:?})",
            request_info,
            status.as_u16(),
            text.len(),
            started.elapsed()
        );
        let result = serde_json::from_str::<TastyApiResponse<T>>(text.as_str()).map_err(|e| {
            // The body is already available at DEBUG above. Keeping it out of the
            // error means a caller that logs or reports the error cannot leak
            // account data it never asked to handle.
            crate::TastyTradeError::Unknown(format!(
                "Failed to parse JSON response for request {}: {}",
                request_info, e
            ))
        })?;

        match result {
            TastyApiResponse::Success(s) => R::from_tasty(s),
            TastyApiResponse::Error { error } => Err(error.into()),
        }
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
    /// Propagates a payload that fails to serialize, and the venue's error
    /// otherwise.
    pub async fn post<R, P, U>(&self, url: U, payload: P) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        P: Serialize,
        U: AsRef<str>,
    {
        let url = format!("{}{}", self.config.base_url, url.as_ref());
        let result = self
            .client
            .post(url)
            .body(serde_json::to_string(&payload)?)
            .send()
            .await?
            .json::<TastyApiResponse<R>>()
            .await?;

        match result {
            TastyApiResponse::Success(s) => Ok(s.data),
            TastyApiResponse::Error { error } => Err(error.into()),
        }
    }

    /// Performs a DELETE.
    ///
    /// **This can mutate account state**, including cancelling an order.
    ///
    /// # Errors
    ///
    /// Propagates the venue's error.
    pub async fn delete<R, U>(&self, url: U) -> TastyResult<R>
    where
        R: DeserializeOwned + Serialize + std::fmt::Debug,
        U: AsRef<str>,
    {
        let url = format!("{}{}", self.config.base_url, url.as_ref());
        let result = self
            .client
            .delete(url)
            .send()
            .await?
            // .inspect_json::<TastyApiResponse<R>, TastyError>(move |text| {
            //     println!("{text}");
            // })
            .json::<TastyApiResponse<R>>()
            .await?;

        match result {
            TastyApiResponse::Success(s) => Ok(s.data),
            TastyApiResponse::Error { error } => Err(error.into()),
        }
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
        let account_number = account_number.into();
        let accounts = self.accounts().await?;
        for account in accounts {
            if account.inner.account.account_number == account_number {
                return Ok(Some(account));
            }
        }
        Ok(None)
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
}
