use std::fmt::Display;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_with::serde_as;
use serde_with::VecSkipError;

#[derive(thiserror::Error, Debug, Deserialize)]
#[serde(untagged)]
pub enum TastyApiResponse<T> {
    Success(Response<T>),
    Error { error: ApiError },
}

#[derive(Debug, Deserialize)]
pub struct Response<T> {
    pub data: T,
    pub context: String,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Pagination {
    pub per_page: usize,
    pub page_offset: usize,
    pub item_offset: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub current_item_count: usize,
    pub previous_link: Option<String>,
    pub next_link: Option<String>,
    pub paging_link_template: Option<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Items<T: DeserializeOwned> {
    // TODO: not this
    #[serde_as(as = "VecSkipError<_>")]
    pub items: Vec<T>,
}

pub struct Paginated<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

#[derive(thiserror::Error, Debug, Deserialize)]
pub struct ApiError {
    pub code: Option<String>,
    pub message: String,
    pub errors: Option<Vec<InnerApiError>>,
}

#[derive(Debug, Deserialize)]
pub struct InnerApiError {
    pub code: Option<String>,
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error {:?}: {}", self.code, self.message)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TastyError {
    #[error("Tastyworks API error")]
    Api(#[from] ApiError),
    #[error("HTTP Error")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON Error")]
    Json(#[from] serde_json::Error),
    #[error("DxFeed Error")]
    DxFeed(#[from] crate::quote_streamer::DxFeedError),
    #[error("Websocket Error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
}

pub type Result<T> = std::result::Result<T, TastyError>;
