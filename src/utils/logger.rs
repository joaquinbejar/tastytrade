//! # Utils Module
//!
//! This module provides a collection of utility functions, structures, and tools designed to simplify and support
//! common tasks across the library. These utilities range from logging, time frame management, testing helpers,
//! and other general-purpose helpers.
//!
//! ## Key Components
//!
//! ### Logger (`logger`)
//!
//! Handles application logging with configurable log levels. It includes safe and idempotent initialization to avoid
//! redundant setups. Useful for debugging, tracing, and monitoring program behavior.
//!
//! **Log Levels:**
//! - `DEBUG`: Detailed debugging information.
//! - `INFO`: General application status information.
//! - `WARN`: Non-critical issues that require attention.
//! - `ERROR`: Significant problems causing failures.
//! - `TRACE`: Fine-grained application execution details.
//!

use tracing_subscriber::FmtSubscriber;
use {std::env, tracing::Level};

/// What an attempt to install this crate's tracing subscriber actually did.
///
/// Process-global logging belongs to the application, not to a library. These
/// helpers are a convenience for binaries and examples that do not want to
/// build a subscriber themselves; when the application already owns one, the
/// attempt reports that and changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggerInit {
    /// This call installed the crate's subscriber.
    Installed,
    /// A subscriber was already installed. Nothing was changed.
    AlreadyInstalled,
    /// Not attempted: this target has no subscriber to install (`wasm32`).
    Unsupported,
}

fn level_from(log_level: &str) -> Level {
    match log_level.trim().to_uppercase().as_str() {
        "DEBUG" => Level::DEBUG,
        "ERROR" => Level::ERROR,
        "WARN" => Level::WARN,
        "TRACE" => Level::TRACE,
        _ => Level::INFO,
    }
}

/// Installs this crate's subscriber at `log_level`, reporting what happened.
///
/// Never panics and never replaces a subscriber the application installed.
/// `set_global_default` is its own guard, so calling this twice is harmless:
/// the second call reports [`LoggerInit::AlreadyInstalled`].
pub fn try_setup_logger_with_level(log_level: &str) -> LoggerInit {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = log_level;
        LoggerInit::Unsupported
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let level = level_from(log_level);
        let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

        match tracing::subscriber::set_global_default(subscriber) {
            Ok(()) => {
                tracing::debug!("Log level set to: {}", level);
                LoggerInit::Installed
            }
            Err(_) => {
                // The other subscriber is live, so this line reaches it.
                tracing::debug!("A tracing subscriber is already installed; leaving it in place");
                LoggerInit::AlreadyInstalled
            }
        }
    }
}

/// Installs this crate's subscriber at the level named by `LOGLEVEL`,
/// reporting what happened. Never panics.
pub fn try_setup_logger() -> LoggerInit {
    // Short-circuit before touching the environment: there is nothing to
    // install on this target, so reading LOGLEVEL would be pure ceremony.
    #[cfg(target_arch = "wasm32")]
    {
        LoggerInit::Unsupported
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let log_level = env::var("LOGLEVEL").unwrap_or_else(|_| "INFO".to_string());
        try_setup_logger_with_level(&log_level)
    }
}

/// Sets up a logger for the application for platforms other than `wasm32`.
///
/// The logger level is determined by the `LOGLEVEL` environment variable.
/// Supported log levels are:
/// - `DEBUG`: Captures detailed debug information.
/// - `ERROR`: Captures error messages.
/// - `WARN`: Captures warnings.
/// - `TRACE`: Captures detailed trace logs.
/// - All other values default to `INFO`, which captures general information.
///
/// **Behavior:**
/// - Repeated calls leave the first subscriber in place.
/// - A subscriber already installed by the application is never replaced.
/// - When targeting `wasm32`, this function is a no-op.
///
/// Prefer [`try_setup_logger`] when the caller wants to know which of those
/// happened. This function discards that outcome and never panics.
pub fn setup_logger() {
    let _ = try_setup_logger();
}

/// Sets up a logger with a user-specified log level for platforms other than `wasm32`.
///
/// **Parameters:**
/// - `log_level`: The desired log level as a string. Supported levels are the same as for `setup_logger`.
///
/// **Behavior:**
/// - Repeated calls leave the first subscriber in place.
/// - A subscriber already installed by the application is never replaced.
/// - When targeting `wasm32`, this function is a no-op.
///
/// Prefer [`try_setup_logger_with_level`] when the caller wants to know which
/// of those happened. This function discards that outcome and never panics.
pub fn setup_logger_with_level(log_level: &str) {
    let _ = try_setup_logger_with_level(log_level);
}

#[cfg(test)]
mod tests_setup_logger {
    use super::*;
    use std::env;
    use tracing::subscriber::set_global_default;
    use tracing_subscriber::FmtSubscriber;

    #[test]
    fn test_logger_initialization_info() {
        unsafe {
            env::set_var("LOGLEVEL", "INFO");
        }
        setup_logger();

        assert!(
            set_global_default(FmtSubscriber::builder().finish()).is_err(),
            "Logger should already be set"
        );
    }

    #[test]
    fn test_logger_initialization_debug() {
        unsafe {
            env::set_var("LOGLEVEL", "DEBUG");
        }
        setup_logger();

        assert!(
            set_global_default(FmtSubscriber::builder().finish()).is_err(),
            "Logger should already be set"
        );
    }

    #[test]
    fn test_logger_initialization_default() {
        unsafe {
            env::remove_var("LOGLEVEL");
        }
        setup_logger();

        assert!(
            set_global_default(FmtSubscriber::builder().finish()).is_err(),
            "Logger should already be set"
        );
    }

    #[test]
    fn test_logger_called_once() {
        unsafe {
            env::set_var("LOGLEVEL", "INFO");
        }

        setup_logger(); // First call should set up the logger
        setup_logger(); // Second call should not re-initialize

        assert!(
            set_global_default(FmtSubscriber::builder().finish()).is_err(),
            "Logger should already be set and should not be reset"
        );
    }
}

#[cfg(test)]
mod tests_setup_logger_bis {
    use super::*;
    use std::sync::Mutex;
    use tracing::subscriber::with_default;
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[derive(Clone)]
    struct TestLayer {
        level: std::sync::Arc<Mutex<Option<Level>>>,
    }

    impl<S> Layer<S> for TestLayer
    where
        S: tracing::Subscriber,
    {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut level = self.level.lock().unwrap();
            *level = Some(*event.metadata().level());
        }
    }

    fn create_test_layer() -> (TestLayer, std::sync::Arc<Mutex<Option<Level>>>) {
        let level = std::sync::Arc::new(Mutex::new(None));
        (
            TestLayer {
                level: level.clone(),
            },
            level,
        )
    }

    #[test]
    fn test_default_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::remove_var("LOGLEVEL");
        }

        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::info!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::INFO));
    }

    #[test]
    fn test_debug_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::set_var("LOGLEVEL", "DEBUG");
        }

        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::debug!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::DEBUG));
        unsafe {
            env::remove_var("LOGLEVEL");
        }
    }

    #[test]
    fn test_error_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::set_var("LOGLEVEL", "ERROR");
        }

        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::error!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::ERROR));
        unsafe {
            env::remove_var("LOGLEVEL");
        }
    }

    #[test]
    fn test_warn_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::set_var("LOGLEVEL", "WARN");
        }
        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::warn!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::WARN));
        unsafe {
            env::remove_var("LOGLEVEL");
        }
    }

    #[test]
    fn test_trace_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::set_var("LOGLEVEL", "TRACE");
        }

        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::trace!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::TRACE));

        unsafe {
            env::remove_var("LOGLEVEL");
        }
    }

    #[test]
    fn test_invalid_log_level() {
        let _lock = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::set_var("LOGLEVEL", "INVALID");
        }

        let (layer, level) = create_test_layer();
        let subscriber = tracing_subscriber::registry().with(layer);

        with_default(subscriber, || {
            setup_logger();
            tracing::info!("Test log");
        });

        assert_eq!(*level.lock().unwrap(), Some(Level::INFO));
        unsafe {
            env::remove_var("LOGLEVEL");
        }
    }
}

// The assertions here are about installing a subscriber, which this target
// does not do; `make wasm-test` would otherwise fail by construction.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests_no_global_seizure {
    use super::*;

    /// The panic this replaces: an application that owns tracing could bring
    /// the process down merely by loading library configuration.
    #[test]
    fn a_second_installation_reports_instead_of_panicking() {
        // Whether this process already has a subscriber depends on test
        // ordering, so assert the property that holds either way: the call
        // returns, and once something is installed every later call agrees.
        let first = try_setup_logger_with_level("WARN");
        assert_ne!(first, LoggerInit::Unsupported);

        for _ in 0..3 {
            assert_eq!(
                try_setup_logger_with_level("DEBUG"),
                LoggerInit::AlreadyInstalled,
                "a subscriber is installed, so no later call may claim otherwise"
            );
        }
    }

    #[test]
    fn level_parsing_is_case_and_whitespace_insensitive() {
        assert_eq!(level_from(" debug "), Level::DEBUG);
        assert_eq!(level_from("Warn"), Level::WARN);
        assert_eq!(level_from("TRACE"), Level::TRACE);
        assert_eq!(level_from("ERROR"), Level::ERROR);
        // Anything unrecognised is INFO, as documented.
        assert_eq!(level_from("verbose"), Level::INFO);
        assert_eq!(level_from(""), Level::INFO);
    }
}
