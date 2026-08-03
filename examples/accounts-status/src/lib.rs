//! Redaction for captured account frames.
//!
//! A frame straight off the socket names the account it is about, and often the
//! person behind it. Committing one as a test fixture without going through
//! this is how an account number ends up in a public repository — which is the
//! one failure this crate spends most of its care avoiding, and a capture tool
//! is exactly where it would happen.
//!
//! Lives in a library rather than in the binary so it can be tested, because a
//! redaction nobody tests is a redaction nobody can trust.

use serde_json::{Map, Value};

/// What every redacted value is replaced with, per field.
///
/// A constant rather than a hash: a fixture is read by people, and
/// `"SENTINEL-5WX00042"` says what it stands for where a digest does not. It is
/// also the value the crate's own tests already assert never reaches a log, so
/// a captured fixture inherits those assertions for free.
const REPLACEMENTS: [(&str, &str); 9] = [
    ("account-number", "SENTINEL-5WX00042"),
    ("user-id", "SENTINEL-user-id"),
    ("username", "SENTINEL-username"),
    ("user-external-id", "SENTINEL-U0001"),
    ("external-id", "SENTINEL-external-id"),
    ("cancel-user-id", "SENTINEL-user-id"),
    ("cancel-username", "SENTINEL-username"),
    ("web-socket-session-id", "SENTINEL-session"),
    ("email", "sentinel@example.com"),
];

/// Replaces every identifying value in `frame`, at any depth.
///
/// Deliberately **key-based and recursive**, not a regular expression over the
/// text: an account number is only recognisable by the field it sits in, and a
/// frame nests — an order carries legs, legs carry fills, and `connect` echoes
/// a list of account numbers under `value`.
///
/// It cannot be complete, and pretending otherwise would be the dangerous part.
/// A field the venue adds tomorrow is not in the list, which is why the tool
/// that uses this prints what it wrote and asks for it to be read before it is
/// committed.
pub fn redact(frame: &mut Value) {
    // `connect` echoes the accounts it subscribed under `value`, which is too
    // generic a key to redact wherever it appears — on a notification it is
    // ordinary data. So it is handled by what the frame *is*, before the
    // key-based pass walks in.
    if let Some(map) = frame.as_object_mut()
        && map.get("action").and_then(Value::as_str) == Some("connect")
        && let Some(Value::Array(accounts)) = map.get_mut("value")
    {
        for account in accounts.iter_mut() {
            *account = Value::String(account_placeholder().to_string());
        }
    }

    match frame {
        Value::Object(map) => {
            redact_object(map);
            for value in map.values_mut() {
                redact(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                redact(item);
            }
        }
        _ => {}
    }
}

/// What an account number is replaced with, wherever it appears.
fn account_placeholder() -> &'static str {
    REPLACEMENTS
        .iter()
        .find(|(key, _)| *key == "account-number")
        .map(|(_, replacement)| *replacement)
        .unwrap_or("SENTINEL-account")
}

fn redact_object(map: &mut Map<String, Value>) {
    for (key, replacement) in REPLACEMENTS {
        let Some(value) = map.get_mut(key) else {
            continue;
        };
        match value {
            // `connect` echoes the accounts it subscribed as an array.
            Value::Array(items) => {
                for item in items.iter_mut() {
                    *item = Value::String(replacement.to_string());
                }
            }
            // A number where a string is expected still identifies somebody:
            // `user-id` arrives as an integer in the venue's own example.
            Value::Null => {}
            _ => *value = Value::String(replacement.to_string()),
        }
    }
}

/// The notification type a frame carries, for naming the file it goes into.
///
/// Falls back to the status action, so an acknowledgement is captured under a
/// name that says what it acknowledged rather than under `unknown`.
pub fn frame_name(frame: &Value) -> String {
    if let Some(kind) = frame.get("type").and_then(Value::as_str) {
        return slug(kind);
    }
    match (
        frame.get("status").and_then(Value::as_str),
        frame.get("action").and_then(Value::as_str),
    ) {
        (Some("error"), Some(action)) => format!("error-{}", slug(action)),
        (Some(_), Some(action)) => format!("status-{}", slug(action)),
        _ => "unrecognised".to_string(),
    }
}

/// `PublicWatchlists` becomes `public-watchlists`; `connect` stays `connect`.
fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "5WX12345";

    fn json(text: &str) -> Value {
        serde_json::from_str(text).expect("valid JSON")
    }

    /// The identifier is nested three deep in a real order frame, so a
    /// top-level pass would leave it.
    #[test]
    fn an_identifier_is_replaced_at_any_depth() {
        let mut frame = json(&format!(
            r#"{{"type":"Order","data":{{
                 "account-number":"{ACCOUNT}",
                 "user-id":99,
                 "username":"coolperson",
                 "legs":[{{"fills":[{{"account-number":"{ACCOUNT}"}}]}}]
               }}}}"#
        ));

        redact(&mut frame);
        let rendered = frame.to_string();

        assert!(
            !rendered.contains(ACCOUNT),
            "an account number survived: {rendered}"
        );
        assert!(!rendered.contains("coolperson"), "{rendered}");
        assert!(
            !rendered.contains("99"),
            "a numeric user id survived: {rendered}"
        );
        assert!(rendered.contains("SENTINEL-5WX00042"), "{rendered}");
        // Everything that is not an identifier is untouched, or the fixture
        // stops being evidence about anything.
        assert_eq!(frame["type"], "Order");
    }

    /// `connect` echoes the accounts it subscribed as an array under a key
    /// too generic to redact everywhere. This is the case a key-based pass
    /// walks straight past, and it is where the account numbers are.
    #[test]
    fn the_accounts_a_connect_echoes_are_replaced_element_by_element() {
        let mut frame =
            json(r#"{"status":"ok","action":"connect","value":["5WX11111","5WX22222"]}"#);

        redact(&mut frame);

        assert_eq!(frame["value"][0], "SENTINEL-5WX00042");
        assert_eq!(frame["value"][1], "SENTINEL-5WX00042");
    }

    /// …and `value` on anything else is ordinary data. Redacting it blindly
    /// would leave a fixture that proves nothing.
    #[test]
    fn a_value_that_is_not_an_account_list_is_left_alone() {
        let mut frame = json(r#"{"status":"ok","action":"heartbeat","value":"kept"}"#);

        redact(&mut frame);

        assert_eq!(frame["value"], "kept");
    }

    /// An absent field is absent, not null-filled: a fixture must keep the
    /// shape the venue actually sent.
    #[test]
    fn a_missing_identifier_is_not_invented() {
        let mut frame = json(r#"{"type":"AccountBalance","data":{"cash-balance":"1.0"}}"#);

        redact(&mut frame);

        assert!(frame["data"].get("account-number").is_none());
        assert_eq!(frame["data"]["cash-balance"], "1.0");
    }

    #[test]
    fn a_frame_is_named_after_what_it_carries() {
        assert_eq!(frame_name(&json(r#"{"type":"Order"}"#)), "order");
        assert_eq!(
            frame_name(&json(r#"{"type":"PublicWatchlists"}"#)),
            "public-watchlists"
        );
        assert_eq!(
            frame_name(&json(r#"{"status":"ok","action":"connect"}"#)),
            "status-connect"
        );
        assert_eq!(
            frame_name(&json(r#"{"status":"error","action":"connect"}"#)),
            "error-connect"
        );
        assert_eq!(frame_name(&json(r#"{"something":"else"}"#)), "unrecognised");
    }
}
