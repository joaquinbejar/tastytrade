use crate::{ApiError, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_with::VecSkipError;
use serde_with::serde_as;
use std::fmt::Display;

#[derive(thiserror::Error, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TastyApiResponse<T: Serialize + std::fmt::Debug> {
    Success(Response<T>),
    Error { error: ApiError },
}

impl Display for TastyApiResponse<String> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TastyApiResponse::Success(response) => write!(f, "{}", response.data),
            TastyApiResponse::Error { error } => write!(f, "{}", error),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T: Serialize + std::fmt::Debug> {
    pub data: T,
    pub context: String,
    pub pagination: Option<Pagination>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct Items<T: DeserializeOwned + Serialize + std::fmt::Debug> {
    #[serde_as(as = "VecSkipError<_>")]
    pub items: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

pub type TastyResult<T> = Result<T, TastyTradeError>;
