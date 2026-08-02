use crate::{TastyTrade, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

const BASE_DEMO_URL: &str = "https://api.cert.tastyworks.com";
const BASE_URL: &str = "https://api.tastyworks.com";

const WEBSOCKET_DEMO_URL: &str = "wss://streamer.cert.tastyworks.com";

const WEBSOCKET_URL: &str = "wss://streamer.tastyworks.com";

/// Configuration structure for the application
/// Handles environment variables and logger setup
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct TastyTradeConfig {
    /// TastyTrade API username/email
    pub username: String,
    /// TastyTrade API password
    #[serde(skip_serializing, default)]
    pub password: String,
    /// Whether to use demo/cert environment
    pub use_demo: bool,
    /// Log level: "INFO", "DEBUG", "WARN", "ERROR", "TRACE"
    pub log_level: String,
    /// Whether to remember login session
    pub remember_me: bool,
    /// Base URL for API requests
    pub base_url: String,
    /// Websocket URL.
    pub websocket_url: String,
}

impl Default for TastyTradeConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl TastyTradeConfig {
    /// Creates a new instance of the type by loading configuration or setup
    /// details from the environment.
    ///
    /// This function is a constructor that initializes the object by calling
    /// `from_env()`, which is expected to handle the process of reading and
    /// setting up values from the environment context (e.g., environment variables).
    ///
    /// # Returns
    /// A new instance of the type.
    ///
    pub fn new() -> Self {
        Self::from_env()
    }

    /// Initialize a new configuration from environment variables.
    ///
    /// The **certification** environment is the default. Production is a
    /// deliberate opt-in through `TASTYTRADE_USE_DEMO=false`, and only a value
    /// that actually parses as `false` selects it — a missing, empty or
    /// misspelled variable resolves to certification, because a typo must not
    /// be what points an order at a funded account.
    pub fn from_env() -> Self {
        #[cfg(not(test))]
        dotenv::dotenv().ok();
        let username = env::var("TASTYTRADE_USERNAME").unwrap_or_default();
        let password = env::var("TASTYTRADE_PASSWORD").unwrap_or_default();
        let log_level = env::var("LOGLEVEL").unwrap_or_else(|_| "INFO".to_string());

        let use_demo = match env::var("TASTYTRADE_USE_DEMO") {
            Ok(raw) => match raw.trim().parse::<bool>() {
                Ok(value) => value,
                Err(_) => {
                    warn!(
                        "TASTYTRADE_USE_DEMO is not a boolean; using the certification environment"
                    );
                    true
                }
            },
            Err(_) => {
                // Absence is the normal case for a new checkout and the safe
                // side, so it is not worth a warning. It is worth a line.
                debug!("TASTYTRADE_USE_DEMO is unset; using the certification environment");
                true
            }
        };
        let remember_me = env::var("TASTYTRADE_REMEMBER_ME")
            .unwrap_or_else(|_| "false".to_string())
            .trim()
            .parse()
            .unwrap_or(false);

        if !use_demo {
            warn!("Using the tastytrade production environment: orders placed here are real");
        }

        Self {
            username,
            password,
            use_demo,
            log_level,
            remember_me,
            base_url: if use_demo {
                BASE_DEMO_URL.to_string()
            } else {
                BASE_URL.to_string()
            },
            websocket_url: if use_demo {
                WEBSOCKET_DEMO_URL.to_string()
            } else {
                WEBSOCKET_URL.to_string()
            },
        }
    }

    /// Load configuration from a JSON file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, TastyTradeError> {
        let contents = fs::read_to_string(path)?;
        let config: TastyTradeConfig = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), TastyTradeError> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Whether both credentials are present.
    ///
    /// Whitespace does not count. `TASTYTRADE_USERNAME=" "` is a shell
    /// accident, not a username, and treating it as one would send an
    /// unusable credential to the venue instead of failing here.
    pub fn has_valid_credentials(&self) -> bool {
        !self.username.trim().is_empty() && !self.password.trim().is_empty()
    }

    /// Creates a TastyTrade client from the configuration.
    ///
    /// # Errors
    ///
    /// Returns [`TastyTradeError::ConfigError`] without making a network
    /// request when the username or password is missing. The error names the
    /// variables to set and never contains their values.
    pub async fn create_client(&self) -> Result<TastyTrade, TastyTradeError> {
        TastyTrade::login(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_default_config() {
        // Ensure environment variables don't interfere with the test
        unsafe {
            env::remove_var("TASTYTRADE_USERNAME");
            env::remove_var("TASTYTRADE_PASSWORD");
            env::remove_var("TASTYTRADE_USE_DEMO");
            env::remove_var("LOGLEVEL");
            env::remove_var("TASTYTRADE_REMEMBER_ME");
        }
        let config = TastyTradeConfig::default();
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
        assert_eq!(config.log_level, "INFO");
        assert!(!config.remember_me);

        // Certification, not production, when nothing says otherwise.
        assert!(
            config.use_demo,
            "an unset environment must not select production"
        );
        assert_eq!(config.base_url, BASE_DEMO_URL);
        assert_eq!(config.websocket_url, WEBSOCKET_DEMO_URL);
    }

    /// Production is reachable only through a value that parses as `false`.
    /// Anything else is a typo, and a typo must not point orders at a funded
    /// account.
    #[test]
    #[serial]
    fn unparseable_use_demo_falls_back_to_certification() {
        for raw in ["", "no", "0", "FALSE!", "prod", "  "] {
            unsafe {
                env::set_var("TASTYTRADE_USE_DEMO", raw);
            }
            let config = TastyTradeConfig::from_env();
            assert!(
                config.use_demo,
                "TASTYTRADE_USE_DEMO={raw:?} must not select production"
            );
            assert_eq!(config.base_url, BASE_DEMO_URL);
        }
        unsafe {
            env::remove_var("TASTYTRADE_USE_DEMO");
        }
    }

    /// Surrounding whitespace is a shell accident, not a different intent.
    #[test]
    #[serial]
    fn production_opt_in_tolerates_surrounding_whitespace() {
        unsafe {
            env::set_var("TASTYTRADE_USE_DEMO", " false ");
        }
        let config = TastyTradeConfig::from_env();
        assert!(!config.use_demo);
        assert_eq!(config.base_url, BASE_URL);
        unsafe {
            env::remove_var("TASTYTRADE_USE_DEMO");
        }
    }

    #[tokio::test]
    #[serial]
    async fn missing_credentials_fail_locally_without_a_request() {
        let config = TastyTradeConfig {
            username: String::new(),
            password: String::new(),
            use_demo: true,
            log_level: "WARN".to_string(),
            remember_me: false,
            // Unroutable on purpose: if the guard ever stops working, this
            // test hangs or fails on connection rather than passing quietly.
            base_url: "http://127.0.0.1:1".to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        };

        let error = config
            .create_client()
            .await
            .expect_err("missing credentials must not reach the venue");

        assert!(
            matches!(error, TastyTradeError::ConfigError(_)),
            "expected a configuration error, got {error:?}"
        );
        let text = format!("{error}");
        assert!(
            text.contains("TASTYTRADE_USERNAME") && text.contains("TASTYTRADE_PASSWORD"),
            "the error must name the variables to set: {text}"
        );
    }

    #[test]
    #[serial]
    fn test_config_from_env() {
        // Set environment variables for testing
        unsafe {
            env::set_var("TASTYTRADE_USERNAME", "test_user");
            env::set_var("TASTYTRADE_PASSWORD", "test_pass");
            env::set_var("TASTYTRADE_USE_DEMO", "true");
            env::set_var("LOGLEVEL", "DEBUG");
            env::set_var("TASTYTRADE_REMEMBER_ME", "true");
        }
        let config = TastyTradeConfig::from_env();
        assert_eq!(config.username, "test_user");
        assert_eq!(config.password, "test_pass");
        assert!(config.use_demo);
        assert!(config.remember_me);
        assert_eq!(config.base_url, BASE_DEMO_URL.to_string());
        assert_eq!(config.websocket_url, WEBSOCKET_DEMO_URL.to_string());

        unsafe {
            // Clean up environment
            env::remove_var("TASTYTRADE_USERNAME");
            env::remove_var("TASTYTRADE_PASSWORD");
            env::remove_var("TASTYTRADE_USE_DEMO");
            env::remove_var("LOGLEVEL");
            env::remove_var("TASTYTRADE_REMEMBER_ME");
        }
    }

    /// A shell accident is not a credential.
    #[test]
    #[serial]
    fn whitespace_is_not_a_credential() {
        let config = TastyTradeConfig {
            username: "   ".to_string(),
            password: "\t\n".to_string(),
            use_demo: true,
            log_level: "WARN".to_string(),
            remember_me: false,
            base_url: BASE_DEMO_URL.to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        };
        assert!(!config.has_valid_credentials());
    }

    #[test]
    #[serial]
    fn test_has_valid_credentials() {
        // Ensure environment variables don't interfere with the test
        unsafe {
            env::remove_var("TASTYTRADE_USERNAME");
            env::remove_var("TASTYTRADE_PASSWORD");
        }
        let mut config = TastyTradeConfig::default();
        assert!(!config.has_valid_credentials());

        config.username = "user".to_string();
        assert!(!config.has_valid_credentials());

        config.password = "pass".to_string();
        assert!(config.has_valid_credentials());
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = TastyTradeConfig {
            username: "test_user".to_string(),
            password: "test_pass".to_string(),
            use_demo: true,
            log_level: "DEBUG".to_string(),
            remember_me: true,
            base_url: BASE_DEMO_URL.to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();

        // Password should be skipped during serialization
        assert!(!json.contains("test_pass"));

        // Create a new config with an empty password
        let mut deserialized: TastyTradeConfig = serde_json::from_str(&json).unwrap();

        // Manually set the password since it's not in the JSON
        deserialized.password = "test_pass".to_string();

        assert_eq!(config.username, deserialized.username);
        assert_eq!(config.password, deserialized.password);
        assert_eq!(config.use_demo, deserialized.use_demo);
        assert_eq!(config.log_level, deserialized.log_level);
        assert_eq!(config.remember_me, deserialized.remember_me);
    }

    #[test]
    #[serial]
    fn test_config_from_env_demo_false() {
        // Clean up any existing environment variables first
        unsafe {
            env::remove_var("TASTYTRADE_USERNAME");
            env::remove_var("TASTYTRADE_PASSWORD");
            env::remove_var("TASTYTRADE_USE_DEMO");
            env::remove_var("LOGLEVEL");
            env::remove_var("TASTYTRADE_REMEMBER_ME");
        }

        // Set environment variables for testing
        unsafe {
            env::set_var("TASTYTRADE_USERNAME", "test_user");
            env::set_var("TASTYTRADE_PASSWORD", "test_pass");
            env::set_var("TASTYTRADE_USE_DEMO", "false");
            env::set_var("LOGLEVEL", "DEBUG");
            env::set_var("TASTYTRADE_REMEMBER_ME", "false");
        }
        let config = TastyTradeConfig::from_env();
        assert_eq!(config.username, "test_user");
        assert_eq!(config.password, "test_pass");
        assert!(!config.use_demo);
        assert!(!config.remember_me);
        assert_eq!(config.base_url, BASE_URL.to_string());
        assert_eq!(config.websocket_url, WEBSOCKET_URL.to_string());

        unsafe {
            // Clean up environment
            env::remove_var("TASTYTRADE_USERNAME");
            env::remove_var("TASTYTRADE_PASSWORD");
            env::remove_var("TASTYTRADE_USE_DEMO");
            env::remove_var("LOGLEVEL");
            env::remove_var("TASTYTRADE_REMEMBER_ME");
        }
    }
}
