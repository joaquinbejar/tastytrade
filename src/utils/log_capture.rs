//! Test-only capture of what this crate logs.
//!
//! The privacy tests assert two things about a log line: that it does not
//! carry the account data it was written about, and that it still says what
//! went wrong. Both need the line to actually reach a subscriber, and that is
//! not guaranteed by `tracing::subscriber::with_default` alone. See
//! [`capture_at`] for the race and the workaround.

use std::io;
use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing::subscriber::NoSubscriber;

/// Everything a capturing subscriber wrote, as bytes behind a lock so the
/// `MakeWriter` clone the subscriber keeps and the test's handle share it.
#[derive(Clone, Default)]
pub(crate) struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    /// The captured text so far.
    pub(crate) fn contents(&self) -> String {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl io::Write for CapturedLogs {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs `f` with a subscriber capturing every event up to `level` on this
/// thread, and returns what it produced together with the captured text.
///
/// # Why there are two dispatchers
///
/// This is a workaround for how `tracing-core` (0.1.36 at the time of
/// writing) caches callsite interest. Each `warn!` caches, process-wide,
/// whether any subscriber wants it; the macro skips the event without
/// consulting the thread's subscriber when that cache says `never`. The
/// cache is computed the first time any thread hits the callsite. When
/// exactly one registered dispatcher is alive, `tracing-core` takes a fast
/// path and computes it from the *calling* thread's dispatcher alone; on a
/// thread with no subscriber that is `NoSubscriber`, which answers `never`.
///
/// So a thread-local capture, on its own, loses the race with any other
/// test that hits the same callsite for the first time while the capture is
/// live: the cache is filled with `never` from the other thread, and the
/// capturing thread's `warn!` is dropped. The capture comes back empty and
/// the test fails on its diagnosability assertion, intermittently, and only
/// while no global subscriber has been installed yet, which is why installing
/// one during the investigation made the failure disappear (#146).
///
/// Keeping a second dispatcher registered for the whole capture takes
/// `tracing-core` off that fast path: every interest computation then folds
/// in all live dispatchers, the capturing one included, so the cache can
/// never say `never` for an event this subscriber wants. `Dispatch::new`
/// is what registers a dispatcher; `Dispatch::none()` is a constant and does
/// not. The second one is a `NoSubscriber`, so it changes nothing else.
pub(crate) fn capture_at<T>(level: Level, f: impl FnOnce() -> T) -> (T, String) {
    let logs = CapturedLogs::default();
    let writer = logs.clone();
    let capturing = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();

    // Registered before the capturing subscriber and dropped after it, so at
    // no point during `f` is the capturing dispatcher the only one alive.
    let _second_dispatcher = tracing::Dispatch::new(NoSubscriber::default());
    let value = tracing::subscriber::with_default(capturing, f);
    drop(_second_dispatcher);

    (value, logs.contents())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::Barrier;
    use std::thread;

    const DIAGNOSTIC: &str = "error at line 1";

    /// One static callsite shared by both threads, shaped like the `warn!`
    /// the privacy tests read.
    #[inline(never)]
    fn emit() {
        tracing::warn!("{DIAGNOSTIC}");
    }

    /// The race from #146, forced: a thread with no subscriber hits the
    /// callsite for the first time in the process while this thread's
    /// capture is live, and only then does this thread emit.
    ///
    /// Only meaningful in a process where no global subscriber has been
    /// installed, which the full suite cannot promise: other tests install
    /// one, and any registered dispatcher is enough to take `tracing-core`
    /// off the fast path being exercised. Hence `#[ignore]`; the test below
    /// runs it in a clean process.
    #[test]
    #[ignore = "run in a clean process by a_first_hit_from_a_foreign_thread_does_not_empty_the_capture"]
    fn foreign_first_hit_inner() {
        let barrier = Arc::new(Barrier::new(2));
        let other = {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait(); // the capture is live
                emit();
                barrier.wait(); // the callsite is registered
            })
        };

        let ((), logs) = capture_at(Level::WARN, || {
            barrier.wait();
            barrier.wait();
            emit();
        });
        other.join().expect("the foreign thread does not panic");

        assert!(
            logs.contains(DIAGNOSTIC),
            "the event this subscriber wanted was dropped; captured: {logs:?}"
        );
    }

    /// Runs `foreign_first_hit_inner` in a fresh copy of this test binary, so
    /// it starts with no global subscriber and no callsite registered, and
    /// relays the verdict. Fails, deterministically, if the second dispatcher
    /// in `capture_at` is removed.
    #[test]
    fn a_first_hit_from_a_foreign_thread_does_not_empty_the_capture() {
        let exe = std::env::current_exe().expect("the test binary knows its own path");
        // `module_path!()` carries the crate name; the harness's test names
        // do not.
        let module = module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, rest)| rest);
        let name = format!("{module}::foreign_first_hit_inner");

        let output = Command::new(exe)
            .args(["--exact", &name, "--ignored", "--test-threads=1"])
            .output()
            .expect("the test binary can be re-run");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stdout.contains("test result: ok. 1 passed"),
            "the clean-process run did not pass exactly one test:\n{stdout}\n{stderr}"
        );
        assert!(
            output.status.success(),
            "the clean-process run failed:\n{stdout}\n{stderr}"
        );
    }
}
