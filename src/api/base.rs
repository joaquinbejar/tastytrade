use crate::{ApiError, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::fmt::Display;
use tracing::warn;

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

#[derive(Debug, Serialize)]
pub struct Items<T: DeserializeOwned + Serialize + std::fmt::Debug> {
    pub items: Vec<T>,
}

impl<'de, T> Deserialize<'de> for Items<T>
where
    T: DeserializeOwned + Serialize + std::fmt::Debug,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ItemsHelper {
            items: Vec<serde_json::Value>,
        }

        let helper = ItemsHelper::deserialize(deserializer)?;
        let mut items = Vec::new();
        let mut error_count = 0;

        for (index, value) in helper.items.into_iter().enumerate() {
            match serde_json::from_value::<T>(value.clone()) {
                Ok(item) => items.push(item),
                Err(e) => {
                    error_count += 1;
                    warn!("🔍 Failed to deserialize item {} in Items<T>: {}", index, e);
                    warn!(
                        "🔍 Raw value: {}",
                        serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| "<invalid json>".to_string())
                    );
                    if error_count <= 3 {
                        // Only log first 3 errors to avoid spam
                        warn!("🔍 Deserialization error details: {:?}", e);
                    }
                }
            }
        }
        
        if error_count > 0 {
            warn!(
                "🔍 Items<T> deserialization summary: {} successful, {} failed",
                items.len(),
                error_count
            );
        }

        Ok(Items { items })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

pub type TastyResult<T> = Result<T, TastyTradeError>;
