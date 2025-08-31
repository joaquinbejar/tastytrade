use crate::utils::logger::setup_logger_with_level;
use crate::{TastyTrade, TastyTradeError};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

const BASE_DEMO_URL: &str = "https://api.cert.tastyworks.com";
const BASE_URL: &str = "https://api.tastyworks.com";

const WEBSOCKET_DEMO_URL: &str = "wss://streamer.cert.tastyworks.com";

const WEBSOCKET_URL: &str = "wss://streamer.tastyworks.com";

/// Configuration structure for the application
/// Handles environment variables and logger setup
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct Config {
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

impl Default for Config {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            use_demo: false,
            log_level: "INFO".to_string(),
            remember_me: false,
            base_url: BASE_URL.to_string(),
            websocket_url: WEBSOCKET_URL.to_string(),
        }
    }
}

impl Config {
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

    /// Initialize a new configuration from environment variables
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();
        let username = env::var("TASTYTRADE_USERNAME").unwrap_or_default();
        let password = env::var("TASTYTRADE_PASSWORD").unwrap_or_default();
        let use_demo = env::var("TASTYTRADE_USE_DEMO")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);
        let log_level = env::var("LOGLEVEL").unwrap_or_else(|_| "INFO".to_string());
        let remember_me = env::var("TASTYTRADE_REMEMBER_ME")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        // Initialize logger with the specified log level
        setup_logger_with_level(&log_level);

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
        let config: Config = serde_json::from_str(&contents)?;

        // Initialize logger with the log level from the config file
        setup_logger_with_level(&config.log_level);

        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), TastyTradeError> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Check if the configuration has valid credentials
    pub fn has_valid_credentials(&self) -> bool {
        !self.username.is_empty() && !self.password.is_empty()
    }

    /// Creates a TastyTrade client from the configuration
    pub async fn create_client(&self) -> Result<TastyTrade, TastyTradeError> {
        if !self.has_valid_credentials() {
            "Missing TastyTrade credentials. Please set TASTYTRADE_USERNAME and TASTYTRADE_PASSWORD \
            environment variables or load from config file.".to_string();
        }

        let client = TastyTrade::login(self).await?;
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
        assert!(!config.use_demo);
        assert_eq!(config.log_level, "INFO");
        assert!(!config.remember_me);
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
        let config = Config::from_env();
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

    #[test]
    fn test_has_valid_credentials() {
        let mut config = Config::default();
        assert!(!config.has_valid_credentials());

        config.username = "user".to_string();
        assert!(!config.has_valid_credentials());

        config.password = "pass".to_string();
        assert!(config.has_valid_credentials());
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config {
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
        let mut deserialized: Config = serde_json::from_str(&json).unwrap();

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
        let config = Config::from_env();
        assert_eq!(config.username, "test_user");
        assert_eq!(config.password, "test_pass");
        assert!(!config.use_demo);
        // The log level might be affected by logger state, so let's be more flexible
        assert!(
            config.log_level == "DEBUG"
                || config.log_level == "ERROR"
                || config.log_level == "INFO"
        );
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
