use std::fmt::Display;

use crate::accounts::{Account, AccountInner, AccountNumber};
use crate::api::base::Items;
use crate::api::base::Paginated;
use crate::api::base::Response;
use crate::api::base::TastyApiResponse;
use crate::api::base::TastyResult;
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

pub trait FromTastyResponse<T: DeserializeOwned + Serialize + std::fmt::Debug> {
    fn from_tasty(resp: Response<T>) -> Self;
}

impl<T: DeserializeOwned + Serialize + std::fmt::Debug> FromTastyResponse<T> for T {
    fn from_tasty(resp: Response<T>) -> Self {
        resp.data
    }
}

impl<T: DeserializeOwned + Serialize + std::fmt::Debug> FromTastyResponse<Items<T>>
    for Paginated<T>
{
    fn from_tasty(resp: Response<Items<T>>) -> Self {
        // Debug logging to understand the conversion
        debug!("🔍 FromTastyResponse conversion:");
        debug!("🔍 resp.data.items.len(): {}", resp.data.items.len());
        debug!("🔍 resp.pagination: {:?}", resp.pagination);

        let pagination = resp
            .pagination
            .expect("Pagination should be present for paginated responses");
        debug!(
            "🔍 pagination.current_item_count: {}",
            pagination.current_item_count
        );
        debug!("🔍 pagination.total_items: {}", pagination.total_items);

        Paginated {
            items: resp.data.items,
            pagination,
        }
    }
}

impl TastyTrade {
    pub async fn default() -> TastyResult<Self> {
        let config = TastyTradeConfig::default();
        Self::login(&config).await
    }

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

        debug!("Login successful for user: {}", creds.user.email);
        let client = Self::create_client(&creds);

        Ok(Self {
            client,
            session_token: creds.session_token,
            config: config.clone(),
        })
    }

    fn create_client(creds: &LoginResponse) -> reqwest::Client {
        let mut headers = HeaderMap::new();

        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&creds.session_token).unwrap(),
        );
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str("application/json").unwrap(),
        );
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str("tastytrade").unwrap(),
        );

        ClientBuilder::new()
            .default_headers(headers)
            .build()
            .expect("Could not create client")
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

        let response: reqwest::Response = if query.is_empty() {
            self.client.get(&full_url).send().await?
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
            self.client.get(url_with_query).send().await?
        };

        let status = response.status();

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(crate::TastyTradeError::Unknown(format!(
                "HTTP {} {} for request {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                request_info,
                error_text
            )));
        }

        let text = response.text().await?;
        debug!("full response for {}: {}", full_request, text);
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
            TastyApiResponse::Success(s) => Ok(R::from_tasty(s)),
            TastyApiResponse::Error { error } => Err(error.into()),
        }
    }

    pub async fn get<T: DeserializeOwned + Serialize + std::fmt::Debug, U: AsRef<str>>(
        &self,
        url: U,
    ) -> TastyResult<T> {
        self.get_with_query(url, &[]).await
    }

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
            .body(serde_json::to_string(&payload).unwrap())
            .send()
            .await?
            .json::<TastyApiResponse<R>>()
            .await?;

        match result {
            TastyApiResponse::Success(s) => Ok(s.data),
            TastyApiResponse::Error { error } => Err(error.into()),
        }
    }

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

    pub async fn accounts(&self) -> TastyResult<Vec<Account<'_>>> {
        let resp: Items<AccountInner> = self.get("/customers/me/accounts").await?;
        Ok(resp
            .items
            .into_iter()
            .map(|inner| Account { inner, tasty: self })
            .collect())
    }

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
