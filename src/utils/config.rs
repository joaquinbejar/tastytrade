use crate::types::oauth::{ClientSecret, RefreshToken};
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
///
/// Authentication is OAuth2. `POST /sessions` was decommissioned on
/// 2026-02-11, so there is no username or password here: what the venue
/// accepts is a client secret and a refresh token created under Manage > My
/// Profile > API on `my.tastytrade.com`.
///
/// Both secrets are `#[serde(skip_serializing)]`, which is what keeps them out
/// of a saved configuration file **and** out of `Debug`, since `DebugPretty`
/// renders through `Serialize`. They are also newtypes that print `***`, so
/// neither protection is load-bearing on its own.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct TastyTradeConfig {
    /// The OAuth application's secret, shown once when the application is
    /// created.
    #[serde(skip_serializing, default)]
    pub client_secret: ClientSecret,
    /// The grant's refresh token. Long-lived: tastytrade's do not expire.
    #[serde(skip_serializing, default)]
    pub refresh_token: RefreshToken,
    /// The OAuth application's public identifier.
    ///
    /// Only needed for the trusted third-party authorization-code flow; the
    /// personal refresh-token flow does not send it.
    #[serde(default)]
    pub client_id: String,
    /// Where tastytrade redirects a customer after they authorize.
    ///
    /// Only needed for the authorization-code flow, and must match one
    /// registered with tastytrade exactly.
    #[serde(default)]
    pub redirect_uri: String,
    /// Whether to use demo/cert environment
    pub use_demo: bool,
    /// Log level: "INFO", "DEBUG", "WARN", "ERROR", "TRACE"
    pub log_level: String,
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
    /// Reads `TASTYTRADE_CLIENT_SECRET` and `TASTYTRADE_REFRESH_TOKEN` for the
    /// personal flow, plus `TASTYTRADE_CLIENT_ID` and
    /// `TASTYTRADE_REDIRECT_URI` for the third-party one.
    ///
    /// The **certification** environment is the default. Production is a
    /// deliberate opt-in through `TASTYTRADE_USE_DEMO=false`, and only a value
    /// that actually parses as `false` selects it — a missing, empty or
    /// misspelled variable resolves to certification, because a typo must not
    /// be what points an order at a funded account.
    pub fn from_env() -> Self {
        #[cfg(not(test))]
        dotenv::dotenv().ok();
        let client_secret =
            ClientSecret::new(env::var("TASTYTRADE_CLIENT_SECRET").unwrap_or_default());
        let refresh_token =
            RefreshToken::new(env::var("TASTYTRADE_REFRESH_TOKEN").unwrap_or_default());
        let client_id = env::var("TASTYTRADE_CLIENT_ID").unwrap_or_default();
        let redirect_uri = env::var("TASTYTRADE_REDIRECT_URI").unwrap_or_default();
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

        if !use_demo {
            warn!("Using the tastytrade production environment: orders placed here are real");
        }

        Self {
            client_secret,
            refresh_token,
            client_id,
            redirect_uri,
            use_demo,
            log_level,
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
    ///
    /// The secrets are not written by [`TastyTradeConfig::save_to_file`], so a
    /// round trip through a file loses them unless the file supplies them by
    /// hand. That is the intended asymmetry: a saved configuration is safe to
    /// keep, and a credential belongs in the environment.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, TastyTradeError> {
        let contents = fs::read_to_string(path)?;
        let config: TastyTradeConfig = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a JSON file
    ///
    /// Writes everything except the client secret and refresh token.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), TastyTradeError> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Which deployment a request built from this configuration will reach.
    ///
    /// Derived from `base_url`, not from `use_demo`. Both fields are public
    /// and can be set independently — a config loaded from JSON or written as
    /// a literal can carry `use_demo: true` beside a production URL — and the
    /// value that decides where the request actually goes is the URL. Anything
    /// that is not the certification host is treated as production, because
    /// that is the answer that fails safe.
    pub fn environment(&self) -> crate::error::Environment {
        if self.base_url.starts_with(BASE_DEMO_URL) {
            crate::error::Environment::Certification
        } else {
            crate::error::Environment::Production
        }
    }

    /// Whether the personal OAuth credentials are both present.
    ///
    /// Whitespace does not count. `TASTYTRADE_CLIENT_SECRET=" "` is a shell
    /// accident, not a secret, and treating it as one would send an unusable
    /// credential to the venue instead of failing here.
    pub fn has_valid_credentials(&self) -> bool {
        !self.client_secret.is_blank() && !self.refresh_token.is_blank()
    }

    /// Creates a TastyTrade client from the configuration.
    ///
    /// # Errors
    ///
    /// Returns [`TastyTradeError::ConfigError`] without making a network
    /// request when the client secret or refresh token is missing. The error
    /// names the variables to set and never contains their values.
    pub async fn create_client(&self) -> Result<TastyTrade, TastyTradeError> {
        TastyTrade::connect(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    /// Every variable this configuration reads, so a test cannot inherit one
    /// from the shell that started it.
    const VARIABLES: [&str; 7] = [
        "TASTYTRADE_CLIENT_SECRET",
        "TASTYTRADE_REFRESH_TOKEN",
        "TASTYTRADE_CLIENT_ID",
        "TASTYTRADE_REDIRECT_URI",
        "TASTYTRADE_USE_DEMO",
        "LOGLEVEL",
        "TASTYTRADE_REMEMBER_ME",
    ];

    fn clear_environment() {
        for name in VARIABLES {
            unsafe {
                env::remove_var(name);
            }
        }
    }

    #[test]
    #[serial]
    fn test_default_config() {
        clear_environment();
        let config = TastyTradeConfig::default();
        assert!(config.client_secret.is_blank());
        assert!(config.refresh_token.is_blank());
        assert!(config.client_id.is_empty());
        assert_eq!(config.log_level, "INFO");

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
        clear_environment();
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
        clear_environment();
    }

    #[tokio::test]
    #[serial]
    async fn missing_credentials_fail_locally_without_a_request() {
        let config = TastyTradeConfig {
            client_secret: ClientSecret::new(""),
            refresh_token: RefreshToken::new(""),
            client_id: String::new(),
            redirect_uri: String::new(),
            use_demo: true,
            log_level: "WARN".to_string(),
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
            text.contains("TASTYTRADE_CLIENT_SECRET") && text.contains("TASTYTRADE_REFRESH_TOKEN"),
            "the error must name the variables to set: {text}"
        );
    }

    #[test]
    #[serial]
    fn test_config_from_env() {
        clear_environment();
        unsafe {
            env::set_var("TASTYTRADE_CLIENT_SECRET", "test_secret");
            env::set_var("TASTYTRADE_REFRESH_TOKEN", "test_refresh");
            env::set_var("TASTYTRADE_CLIENT_ID", "test_client");
            env::set_var("TASTYTRADE_REDIRECT_URI", "https://app.example.com/cb");
            env::set_var("TASTYTRADE_USE_DEMO", "true");
            env::set_var("LOGLEVEL", "DEBUG");
        }
        let config = TastyTradeConfig::from_env();
        assert_eq!(config.client_secret.expose_secret(), "test_secret");
        assert_eq!(config.refresh_token.expose_secret(), "test_refresh");
        assert_eq!(config.client_id, "test_client");
        assert_eq!(config.redirect_uri, "https://app.example.com/cb");
        assert!(config.use_demo);
        assert_eq!(config.base_url, BASE_DEMO_URL.to_string());
        assert_eq!(config.websocket_url, WEBSOCKET_DEMO_URL.to_string());

        clear_environment();
    }

    /// `TASTYTRADE_REMEMBER_ME` selected a behaviour of the retired session
    /// API. Reading it now would suggest it still does something.
    #[test]
    #[serial]
    fn the_retired_remember_me_variable_is_not_read() {
        clear_environment();
        unsafe {
            env::set_var("TASTYTRADE_REMEMBER_ME", "true");
        }

        let rendered = format!("{:?}", TastyTradeConfig::from_env());
        assert!(
            !rendered.contains("remember"),
            "a retired setting must not reappear in the configuration: {rendered}"
        );
        clear_environment();
    }

    /// A shell accident is not a credential.
    #[test]
    #[serial]
    fn whitespace_is_not_a_credential() {
        let config = TastyTradeConfig {
            client_secret: ClientSecret::new("   "),
            refresh_token: RefreshToken::new("\t\n"),
            client_id: String::new(),
            redirect_uri: String::new(),
            use_demo: true,
            log_level: "WARN".to_string(),
            base_url: BASE_DEMO_URL.to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        };
        assert!(!config.has_valid_credentials());
    }

    #[test]
    #[serial]
    fn test_has_valid_credentials() {
        clear_environment();
        let mut config = TastyTradeConfig::default();
        assert!(!config.has_valid_credentials());

        config.client_secret = ClientSecret::new("secret");
        assert!(!config.has_valid_credentials());

        config.refresh_token = RefreshToken::new("refresh");
        assert!(config.has_valid_credentials());
    }

    /// The secrets stay out of the serialized form, and therefore out of
    /// `Debug` and `Display` too, because both render through `Serialize`.
    #[test]
    fn test_serialize_deserialize() {
        let config = TastyTradeConfig {
            client_secret: ClientSecret::new("SENTINEL-client-secret-3Qv7"),
            refresh_token: RefreshToken::new("SENTINEL-refresh-token-8Hb2"),
            client_id: "client-abc".to_string(),
            redirect_uri: "https://app.example.com/cb".to_string(),
            use_demo: true,
            log_level: "DEBUG".to_string(),
            base_url: BASE_DEMO_URL.to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        };

        let json = serde_json::to_string(&config).expect("the config serializes");
        for rendered in [json.clone(), format!("{config:?}"), format!("{config}")] {
            assert!(
                !rendered.contains("SENTINEL"),
                "a secret escaped: {rendered}"
            );
        }
        // The public half survives, which is the point of saving one at all.
        assert!(json.contains("client-abc"), "{json}");

        let deserialized: TastyTradeConfig =
            serde_json::from_str(&json).expect("the config round-trips");
        assert_eq!(config.client_id, deserialized.client_id);
        assert_eq!(config.use_demo, deserialized.use_demo);
        assert_eq!(config.log_level, deserialized.log_level);
        assert!(
            deserialized.client_secret.is_blank(),
            "a saved configuration must not be able to carry the secret back"
        );
    }

    #[test]
    #[serial]
    fn test_config_from_env_demo_false() {
        clear_environment();
        unsafe {
            env::set_var("TASTYTRADE_CLIENT_SECRET", "test_secret");
            env::set_var("TASTYTRADE_REFRESH_TOKEN", "test_refresh");
            env::set_var("TASTYTRADE_USE_DEMO", "false");
            env::set_var("LOGLEVEL", "DEBUG");
        }
        let config = TastyTradeConfig::from_env();
        assert_eq!(config.client_secret.expose_secret(), "test_secret");
        assert!(!config.use_demo);
        assert_eq!(config.base_url, BASE_URL.to_string());
        assert_eq!(config.websocket_url, WEBSOCKET_URL.to_string());

        clear_environment();
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    use crate::error::Environment;

    fn config_with(base_url: &str, use_demo: bool) -> TastyTradeConfig {
        TastyTradeConfig {
            client_secret: ClientSecret::new("secret"),
            refresh_token: RefreshToken::new("refresh"),
            client_id: String::new(),
            redirect_uri: String::new(),
            use_demo,
            log_level: "WARN".to_string(),
            base_url: base_url.to_string(),
            websocket_url: WEBSOCKET_DEMO_URL.to_string(),
        }
    }

    /// Both fields are public, so a config built as a literal or loaded from
    /// JSON can carry a flag that disagrees with the URL. The URL is what the
    /// request actually uses, so it is what the reported environment follows.
    #[test]
    fn the_url_decides_not_the_flag() {
        assert_eq!(
            config_with(BASE_URL, true).environment(),
            Environment::Production,
            "use_demo must not relabel a production URL as certification"
        );
        assert_eq!(
            config_with(BASE_DEMO_URL, false).environment(),
            Environment::Certification
        );
    }

    /// An unrecognised host is reported as production, because that is the
    /// answer that makes a caller careful rather than complacent.
    #[test]
    fn an_unknown_host_is_reported_as_production() {
        assert_eq!(
            config_with("http://127.0.0.1:8080", true).environment(),
            Environment::Production
        );
    }
}
