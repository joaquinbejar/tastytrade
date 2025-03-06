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

use std::sync::Once;
use tracing_subscriber::FmtSubscriber;
use {std::env, tracing::Level};

static INIT: Once = Once::new();

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
/// - Concurrent calls to this function result in the logger being initialized only once.
/// - When targeting `wasm32`, this function is effectively a no-op.
///
/// # Panics
/// This function panics if setting the default subscriber fails.
pub fn setup_logger() {
    #[cfg(not(target_arch = "wasm32"))]
    INIT.call_once(|| {
        let log_level = env::var("LOGLEVEL")
            .unwrap_or_else(|_| "INFO".to_string())
            .to_uppercase();

        let level = match log_level.as_str() {
            "DEBUG" => Level::DEBUG,
            "ERROR" => Level::ERROR,
            "WARN" => Level::WARN,
            "TRACE" => Level::TRACE,
            _ => Level::INFO,
        };

        let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("Error setting default subscriber");

        tracing::debug!("Log level set to: {}", level);
    });
}

/// Sets up a logger with a user-specified log level for platforms other than `wasm32`.
///
/// **Parameters:**
/// - `log_level`: The desired log level as a string. Supported levels are the same as for `setup_logger`.
///
/// **Behavior:**
/// - Concurrent calls to this function result in the logger being initialized only once.
/// - When targeting `wasm32`, this function is a no-op.
///
/// # Panics
/// This function panics if setting the default subscriber fails.
pub fn setup_logger_with_level(log_level: &str) {
    INIT.call_once(|| {
        let log_level = log_level.to_uppercase();

        let level = match log_level.as_str() {
            "DEBUG" => Level::DEBUG,
            "ERROR" => Level::ERROR,
            "WARN" => Level::WARN,
            "TRACE" => Level::TRACE,
            _ => Level::INFO,
        };

        let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("Error setting default subscriber");

        tracing::debug!("Log level set to: {}", level);
    });
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
