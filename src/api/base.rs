use crate::{ApiError, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::fmt::Display;
use tracing::{debug, warn};

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
                    // The serde error names the offending field, which is what makes
                    // these diagnosable. The value itself is user data (account
                    // numbers, balances, order contents) and stays at DEBUG, where
                    // it is only emitted if the consumer deliberately asks for it.
                    warn!("failed to deserialize item {} in Items<T>: {}", index, e);
                    debug!(
                        "raw item {}: {}",
                        index,
                        serde_json::to_string(&value)
                            .unwrap_or_else(|_| "<invalid json>".to_string())
                    );
                }
            }
        }

        if error_count > 0 {
            warn!(
                "Items<T> deserialization: {} succeeded, {} failed",
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing::Level;

    /// An account-shaped item that the certification environment cannot satisfy:
    /// `is-test-drive` is absent from its payload, so the item fails to
    /// deserialize and takes the logging path under test.
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    struct StrictAccount {
        account_number: String,
        nickname: String,
        is_test_drive: bool,
    }

    /// Shaped like a real `/customers/me/accounts` response, with the two values
    /// that must never reach a log: the account number and the nickname.
    const ACCOUNT_NUMBER: &str = "5WX12345";
    const NICKNAME: &str = "Retirement";
    const PAYLOAD: &str = r#"{"items":[{"account-number":"5WX12345","nickname":"Retirement","margin-or-cash":"Margin"}]}"#;

    #[derive(Clone, Default)]
    struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

    impl CapturedLogs {
        fn contents(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    impl io::Write for CapturedLogs {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Deserializes PAYLOAD with the subscriber capturing everything up to
    /// `max_level`, and returns what was logged.
    fn logs_for(max_level: Level) -> (Vec<StrictAccount>, String) {
        let logs = CapturedLogs::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(max_level)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();

        let items = tracing::subscriber::with_default(subscriber, || {
            serde_json::from_str::<Items<StrictAccount>>(PAYLOAD)
                .expect("the envelope itself is well formed")
                .items
        });

        (items, logs.contents())
    }

    #[test]
    #[serial]
    fn failed_item_is_skipped_not_fatal() {
        let (items, _) = logs_for(Level::WARN);
        assert!(items.is_empty(), "the only item cannot deserialize");
    }

    #[test]
    #[serial]
    fn warn_level_names_the_field_but_never_the_payload() {
        let (_, logs) = logs_for(Level::WARN);

        // Diagnosable: the missing field is what makes this actionable.
        assert!(
            logs.contains("is-test-drive"),
            "the serde error must survive: {logs}"
        );
        assert!(
            logs.contains("1 failed"),
            "the summary must survive: {logs}"
        );

        // Private: nothing from the payload may appear at WARN.
        assert!(
            !logs.contains(ACCOUNT_NUMBER),
            "account number leaked into WARN logs: {logs}"
        );
        assert!(
            !logs.contains(NICKNAME),
            "nickname leaked into WARN logs: {logs}"
        );
        assert!(
            !logs.contains("Margin"),
            "raw payload leaked into WARN logs: {logs}"
        );
    }

    #[test]
    #[serial]
    fn debug_level_still_has_the_payload_for_diagnosis() {
        let (_, logs) = logs_for(Level::DEBUG);

        assert!(
            logs.contains(ACCOUNT_NUMBER),
            "the payload must remain available when DEBUG is asked for: {logs}"
        );
    }
}
