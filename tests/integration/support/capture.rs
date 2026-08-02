//! Tracing capture, so a test can assert on what the crate did and did not
//! write to a consumer's logs.

use std::io;
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing::instrument::WithSubscriber;

/// An in-memory sink for `tracing` output.
#[derive(Clone, Default)]
pub struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    /// Everything written so far.
    pub fn contents(&self) -> String {
        String::from_utf8_lossy(
            &self
                .0
                .lock()
                .expect("the capture mutex is never poisoned in tests"),
        )
        .into_owned()
    }

    /// Fails with the captured text when `needle` appears. The message carries
    /// the log so a failure explains itself without a second run.
    pub fn assert_absent(&self, needle: &str, what: &str) {
        let logs = self.contents();
        assert!(
            !logs.contains(needle),
            "{what} leaked into the logs.\n--- captured ---\n{logs}"
        );
    }

    /// Fails when `needle` does not appear.
    pub fn assert_present(&self, needle: &str, what: &str) {
        let logs = self.contents();
        assert!(
            logs.contains(needle),
            "{what} is missing from the logs.\n--- captured ---\n{logs}"
        );
    }
}

impl io::Write for CapturedLogs {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("the capture mutex is never poisoned in tests")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs `body` with everything up to `max_level` captured, and returns what it
/// produced alongside the future's output.
///
/// The subscriber is attached to the future rather than to the thread.
/// `set_default` installs a *thread-local* dispatcher, so holding its guard
/// across an `.await` captures whatever happens to run on the original thread
/// while the future itself may be polled somewhere else. Attaching it to the
/// task makes the capture follow the work, on any runtime flavour.
///
/// It is also scoped rather than global, so tests do not fight each other over
/// process-wide logging — the very thing the library stopped doing.
pub async fn capture_logs_at<F, T>(max_level: Level, body: F) -> (T, CapturedLogs)
where
    F: std::future::Future<Output = T>,
{
    let logs = CapturedLogs::default();
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(max_level)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();

    let out = body.with_subscriber(subscriber).await;

    (out, logs)
}
