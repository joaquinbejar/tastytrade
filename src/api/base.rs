use crate::{ApiError, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::fmt::Display;
use tracing::{debug, warn};

/// The two shapes a tastytrade response can take.
///
/// Untagged, so the discriminant is the shape of the body rather than a field.
/// One consequence is worth knowing: untagged deserialization discards the
/// inner error and reports "data did not match any variant", so a failure
/// inside `T` cannot explain itself through this enum.
#[derive(thiserror::Error, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TastyApiResponse<T: Serialize + std::fmt::Debug> {
    /// The venue answered with data.
    Success(Response<T>),
    /// The venue answered with an error document.
    Error {
        /// The broker's own code and message.
        error: ApiError,
    },
}

impl Display for TastyApiResponse<String> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TastyApiResponse::Success(response) => write!(f, "{}", response.data),
            TastyApiResponse::Error { error } => write!(f, "{}", error),
        }
    }
}

/// A successful response envelope.
#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T: Serialize + std::fmt::Debug> {
    /// The payload.
    pub data: T,
    /// The endpoint the venue believes it answered.
    ///
    /// Venue-supplied and usually the request path, so on an account-scoped
    /// endpoint it contains the account number. Redact it before it reaches a
    /// log or an error.
    pub context: String,
    /// Present only on paginated endpoints.
    pub pagination: Option<Pagination>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Where a page sits in a paginated listing.
pub struct Pagination {
    /// Items requested per page.
    pub per_page: usize,
    /// Zero-based index of this page.
    pub page_offset: usize,
    /// Index of the first item on this page within the whole listing.
    pub item_offset: usize,
    /// Items in the listing as a whole.
    pub total_items: usize,
    /// Pages in the listing as a whole.
    pub total_pages: usize,
    /// Items on this page, which can be zero on a page the venue still counts.
    pub current_item_count: usize,
    /// Link to the previous page, when there is one.
    pub previous_link: Option<String>,
    /// Link to the next page, when there is one.
    pub next_link: Option<String>,
    /// Template the venue offers for building page links.
    pub paging_link_template: Option<String>,
}

/// A venue listing, tolerant of items this crate cannot model yet.
///
/// The venue adds fields without notice, so one unparseable item must not lose
/// a listing of five thousand. What the caller still needs is to be able to
/// tell two situations apart that used to look identical: the venue genuinely
/// returned nothing, and everything it returned was unparseable. The first is
/// normal; the second is a defect in this crate.
#[derive(Debug, Serialize)]
pub struct Items<T: DeserializeOwned + Serialize + std::fmt::Debug> {
    /// The items that decoded successfully.
    pub items: Vec<T>,
    /// How many items were dropped because they could not be decoded.
    ///
    /// Zero on a healthy response. Non-zero means this crate's model has
    /// drifted from what the venue sends, and the dropped items are invisible
    /// to everything downstream.
    ///
    /// Client-side decode metadata, never part of the wire shape, so it is not
    /// serialized: round-tripping an `Items` value must not invent a field the
    /// venue does not have.
    #[serde(skip_serializing)]
    pub skipped: usize,
}

/// How many per-item deserialization failures are worth a warning. Schema drift
/// fails every item in a listing at once, and a listing can hold thousands, so
/// past the first few the summary carries the scope and the rest go to DEBUG.
const MAX_ITEM_WARNINGS: usize = 3;

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
                    // The serde error is not metadata: a type mismatch renders as
                    // `invalid type: string "Retirement", expected a boolean`, so
                    // the message itself carries the rejected value. WARN gets the
                    // value-free classification and position; the error and the raw
                    // item stay at DEBUG, where they are only emitted if the
                    // consumer deliberately asks for them.
                    if error_count <= MAX_ITEM_WARNINGS {
                        warn!(
                            "failed to deserialize item {} in Items<T>: {:?} error at line {}, column {}; enable DEBUG for details",
                            index,
                            e.classify(),
                            e.line(),
                            e.column()
                        );
                    }
                    debug!("item {} serde error: {}", index, e);
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

        Ok(Items {
            items,
            skipped: error_count,
        })
    }
}

impl<T: DeserializeOwned + Serialize + std::fmt::Debug> Items<T> {
    /// The decoded items, or an error when every item was dropped.
    ///
    /// Tolerating an unparseable item is right; silently answering "nothing
    /// here" when *no* item could be decoded is not. That state is a defect in
    /// this crate rather than a thin response, and it is what made a single
    /// missing field read as an authentication problem.
    ///
    /// The check cannot live in the `Deserialize` implementation: the response
    /// envelope is an untagged enum, and untagged deserialization discards the
    /// inner error in favour of "data did not match any variant", so the
    /// explanation would never reach the caller.
    pub fn into_items(self) -> TastyResult<Vec<T>> {
        if self.items.is_empty() && self.skipped > 0 {
            return Err(TastyTradeError::Unknown(format!(
                "all {} item(s) in the listing failed to deserialize; this crate's model \
                 does not match what the venue returned (raise the log level for diagnostics)",
                self.skipped
            )));
        }
        Ok(self.items)
    }
}

/// One page of a paginated listing.
#[derive(Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    /// The items on this page. Unparseable ones are already dropped; see
    /// [`Items::into_items`] for what that means.
    pub items: Vec<T>,
    /// Where this page sits in the listing.
    pub pagination: Pagination,
}

/// The result type every fallible call in this crate returns.
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
    /// One item that decodes and one that cannot. The healthy item is what
    /// keeps this in the tolerated-skip path rather than the all-failed one.
    const PAYLOAD: &str = r#"{"items":[
        {"account-number":"5WX00001","nickname":"Healthy","is-test-drive":false},
        {"account-number":"5WX12345","nickname":"Retirement","margin-or-cash":"Margin"}
    ]}"#;

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

    /// Deserializes `payload` with the subscriber capturing everything up to
    /// `max_level`, and returns what was logged.
    fn logs_for(payload: &str, max_level: Level) -> (Items<StrictAccount>, String) {
        let logs = CapturedLogs::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(max_level)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();

        let items = tracing::subscriber::with_default(subscriber, || {
            serde_json::from_str::<Items<StrictAccount>>(payload)
                .expect("at least one item decodes in these fixtures")
        });

        (items, logs.contents())
    }

    #[test]
    #[serial]
    fn a_failed_item_is_skipped_and_counted() {
        let (items, _) = logs_for(PAYLOAD, Level::WARN);

        assert_eq!(items.items.len(), 1, "the healthy item survives");
        assert_eq!(
            items.skipped, 1,
            "the caller must be able to see that something was dropped"
        );
    }

    /// The distinction this type could not express before: a venue that
    /// returned nothing, and a venue whose every item this crate failed to
    /// model. The second is a defect here, and returning an empty list for it
    /// is what made a missing field read as an authentication problem.
    #[test]
    #[serial]
    fn an_empty_listing_and_an_unparseable_one_are_not_the_same() {
        let empty = serde_json::from_str::<Items<StrictAccount>>(r#"{"items":[]}"#)
            .expect("an empty listing is a normal response");
        assert_eq!(empty.skipped, 0);
        assert!(
            empty
                .into_items()
                .expect("nothing was dropped, so nothing is wrong")
                .is_empty()
        );

        let all_failed = serde_json::from_str::<Items<StrictAccount>>(
            r#"{"items":[{"account-number":"5WX1","nickname":"a"}]}"#,
        )
        .expect("decoding tolerates the failure; reporting it is into_items' job")
        .into_items()
        .expect_err("a listing where nothing decodes is an error");

        let rendered = all_failed.to_string();
        assert!(
            rendered.contains("all 1 item(s)"),
            "the error must say how many were lost: {rendered}"
        );
        assert!(
            !rendered.contains("5WX1"),
            "the error must not carry the payload: {rendered}"
        );
    }

    #[test]
    #[serial]
    fn warn_level_is_diagnosable_without_the_payload() {
        let (_, logs) = logs_for(PAYLOAD, Level::WARN);

        // Diagnosable: which item, what class of failure, where in the body.
        assert!(
            logs.contains("failed to deserialize item 1"),
            "the failing item must be identified: {logs}"
        );
        assert!(
            logs.contains("Data error"),
            "the serde category must survive: {logs}"
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

    /// A missing field renders without any value, but a type mismatch renders as
    /// `invalid type: string "5WX12345", expected a boolean` — the serde error
    /// carries the rejected value, so it cannot be logged at WARN either.
    #[test]
    #[serial]
    fn a_type_mismatch_does_not_leak_the_rejected_value() {
        let payload = format!(
            r#"{{"items":[
                {{"account-number":"5WX00001","nickname":"Healthy","is-test-drive":false}},
                {{"account-number":"{ACCOUNT_NUMBER}","nickname":"{NICKNAME}","is-test-drive":"{ACCOUNT_NUMBER}"}}
            ]}}"#
        );

        let (parsed, warn_logs) = logs_for(&payload, Level::WARN);
        assert_eq!(parsed.skipped, 1, "the second item fails on the boolean");
        assert!(
            !warn_logs.contains(ACCOUNT_NUMBER),
            "the rejected value reached WARN through the serde error: {warn_logs}"
        );

        // Still diagnosable one level down.
        let (_, debug_logs) = logs_for(&payload, Level::DEBUG);
        assert!(
            debug_logs.contains("invalid type"),
            "the full serde error must remain at DEBUG: {debug_logs}"
        );
    }

    #[test]
    #[serial]
    fn debug_level_still_has_the_payload_for_diagnosis() {
        let (_, logs) = logs_for(PAYLOAD, Level::DEBUG);

        assert!(
            logs.contains(ACCOUNT_NUMBER),
            "the payload must remain available when DEBUG is asked for: {logs}"
        );
    }

    /// Schema drift fails every item at once. A listing can hold thousands, so
    /// the per-item warning is capped and the summary carries the scope.
    #[test]
    #[serial]
    fn per_item_warnings_are_capped_but_the_summary_is_not() {
        let mut items = vec![
            r#"{"account-number":"5WX00001","nickname":"Healthy","is-test-drive":false}"#
                .to_string(),
        ];
        items.extend(
            (0..10)
                .map(|i| format!(r#"{{"account-number":"5WX0000{i}","nickname":"Account {i}"}}"#)),
        );
        let payload = format!(r#"{{"items":[{}]}}"#, items.join(","));

        let (parsed, warn_logs) = logs_for(&payload, Level::WARN);
        assert_eq!(parsed.skipped, 10, "ten of the eleven items fail");

        let warned = warn_logs.matches("failed to deserialize item").count();
        assert_eq!(
            warned, MAX_ITEM_WARNINGS,
            "ten failures must not produce ten warnings: {warn_logs}"
        );
        assert!(
            warn_logs.contains("1 succeeded, 10 failed"),
            "the summary must still report every failure: {warn_logs}"
        );

        // The suppressed failures are still diagnosable when asked for.
        let (_, debug_logs) = logs_for(&payload, Level::DEBUG);
        assert_eq!(
            debug_logs.matches("serde error:").count(),
            10,
            "DEBUG must keep every failure: {debug_logs}"
        );
    }
}

#[cfg(test)]
mod wire_shape_tests {
    use super::*;

    /// `skipped` is decode metadata this crate keeps for the caller. Emitting
    /// it would invent a broker field in anything that re-serializes a listing.
    #[test]
    fn the_skipped_count_is_not_part_of_the_wire_shape() {
        let items = Items::<String> {
            items: vec!["a".to_string()],
            skipped: 3,
        };

        let json = serde_json::to_string(&items).expect("Items serializes");
        assert_eq!(json, r#"{"items":["a"]}"#);
    }
}
