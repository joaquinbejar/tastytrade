use serde::Deserialize;
use std::error::Error;
use std::fmt::{self, Display};
use std::io;

/// Represents errors that can occur during interactions with DxFeed.
///
/// This enum provides variants for different types of errors related to DxFeed.
/// Currently, it only includes a variant for connection errors.
#[derive(Debug)]
pub enum DxFeedError {
    /// Represents an error encountered while creating a connection to DxFeed.
    /// This can occur due to various reasons, such as network issues or invalid
    /// connection parameters.
    CreateConnectionError,
}

impl Display for DxFeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DxFeed error: {:?}", self)
    }
}

impl Error for DxFeedError {}

/// Represents an error returned by the Tastytrade API.
///
/// This struct provides detailed information about errors encountered when interacting with the Tastytrade API.  It includes an optional error code, a human-readable error message, and an optional list of inner errors for more specific diagnostic information.
#[derive(Debug, Deserialize)]
pub struct ApiError {
    /// An optional error code. This can be used for programmatic identification of specific errors.
    pub code: Option<String>,
    /// A human-readable error message. This provides a description of the error that occurred.
    pub message: String,
    /// An optional list of inner errors. These provide more detailed information about the error, such as specific validation failures.
    pub errors: Option<Vec<InnerApiError>>,
}

/// Represents an inner API error.  This struct is typically nested within a top-level `ApiError` to provide more detailed error information.
#[derive(Debug, Deserialize)]
pub struct InnerApiError {
    /// An optional error code.  This can be used for programmatic identification of specific errors.
    pub code: Option<String>,
    /// A human-readable error message.  This provides a description of the error that occurred.
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error {:?}: {}", self.code, self.message)
    }
}

impl Error for ApiError {}

/// Represents errors that can occur within the Tastytrade API client.
#[derive(Debug)]
pub enum TastyTradeError {
    /// Represents an error returned from the Tastytrade API.  This variant contains an `ApiError` struct, which provides details about the API error, including an error code and message.
    Api(ApiError),
    /// Represents an HTTP error during communication with the Tastytrade API.  This variant wraps a `reqwest::Error`, which provides details about the underlying HTTP error.
    Http(reqwest::Error),
    /// Represents an error during JSON serialization or deserialization.  This variant wraps a `serde_json::Error`, which provides details about the JSON error.
    Json(serde_json::Error),
    /// Represents an error originating from the DxFeed data stream.  This variant contains a `DxFeedError` enum, which provides details about the specific DxFeed error.
    DxFeed(DxFeedError),
    /// Represents an error that occurred during WebSocket communication, often related to real-time data streaming. This variant wraps a `tokio_tungstenite::tungstenite::Error`, providing details about the WebSocket error.
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// Represents an I/O error. This variant wraps a standard `io::Error`, providing details about the I/O operation that failed.
    Io(io::Error),
    /// Represents an authentication error. This variant contains a `String` describing the authentication failure.
    Auth(String),
    /// Represents a connection error, typically during the initial connection establishment phase.  This variant contains a `String` describing the connection failure.
    Connection(String),
    /// Represents an error related to real-time data streaming after a successful connection. This variant contains a `String` describing the streaming error.
    Streaming(String),
    /// Represents an unknown or unexpected error. This variant contains a `String` describing the error.
    Unknown(String),
    /// Represents an error within the client configuration. This variant contains a `String` describing the configuration error.
    ConfigError(String),
}

impl Display for TastyTradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api(err) => write!(f, "API error: {}", err),
            Self::Http(err) => write!(f, "HTTP error: {}", err),
            Self::Json(err) => write!(f, "JSON parsing error: {}", err),
            Self::DxFeed(err) => write!(f, "DxFeed error: {}", err),
            Self::WebSocket(err) => write!(f, "WebSocket error: {}", err),
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Auth(msg) => write!(f, "Authentication failed: {}", msg),
            Self::Connection(msg) => write!(f, "Connection error: {}", msg),
            Self::Streaming(msg) => write!(f, "Streaming error: {}", msg),
            Self::Unknown(msg) => write!(f, "Unknown error: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl Error for TastyTradeError {
    /// Returns the underlying source of the error if available.
    ///
    /// Some errors, such as `Auth`, `Connection`, `Streaming`, `Unknown`, and `ConfigError` do not have
    /// an underlying source error.  This is because these errors are generated internally within the
    /// library and do not wrap external errors.  In these cases, this function will return `None`.
    ///
    /// For errors that wrap an external error, such as `Api`, `Http`, `Json`, `DxFeed`, `WebSocket`, and `Io`,
    /// this function will return a reference to the underlying error as a trait object `&(dyn Error + 'static)`.
    /// This allows access to the original error information for debugging and handling.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::error::Error;
    /// use tastytrade::TastyTradeError;
    /// use std::io;
    ///
    /// let error = TastyTradeError::Io(io::Error::new(io::ErrorKind::Other, "IO error"));
    ///
    /// if let Some(source) = error.source() {
    ///     println!("Source error: {}", source);
    /// } else {
    ///     println!("No source error available.");
    /// }
    ///
    /// let error = TastyTradeError::Auth("Authentication failed".to_string());
    ///
    /// if let Some(source) = error.source() {
    ///     println!("Source error: {}", source);
    /// } else {
    ///     println!("No source error available.");
    /// }
    /// ```
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Api(err) => Some(err),
            Self::Http(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::DxFeed(err) => Some(err),
            Self::WebSocket(err) => Some(err.as_ref()),
            Self::Io(err) => Some(err),
            Self::Auth(_) => None,
            Self::Connection(_) => None,
            Self::Streaming(_) => None,
            Self::Unknown(_) => None,
            Self::ConfigError(_) => None,
        }
    }
}

impl From<ApiError> for TastyTradeError {
    /// Converts an `ApiError` into a `TastyTradeError`.
    ///
    /// This function implements the `From` trait for converting an `ApiError`
    /// into a `TastyTradeError::Api` variant. This allows for seamless error
    /// propagation and handling within the library.
    ///
    /// # Arguments
    ///
    /// * `err` - The `ApiError` to be converted.
    ///
    /// # Returns
    ///
    /// A `TastyTradeError::Api` containing the provided `ApiError`.
    ///
    /// # Example
    ///
    /// ```
    /// use tastytrade::{ApiError, TastyTradeError};
    ///
    /// let api_error = ApiError {
    ///     code: Some("400".to_string()),
    ///     message: "Bad Request".to_string(),
    ///     errors: None,
    /// };
    ///
    /// let tasty_error: TastyTradeError = api_error.into();
    ///
    /// assert!(matches!(tasty_error, TastyTradeError::Api(_)));
    /// ```
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

impl From<reqwest::Error> for TastyTradeError {
    /// Converts a `reqwest::Error` into a `TastyTradeError`.
    ///
    /// This function maps a `reqwest::Error`, which represents an error during an HTTP request,
    /// into a `TastyTradeError::Http` variant. This allows the library to handle HTTP errors
    /// consistently with other error types within the `TastyTradeError` enum.
    ///
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err)
    }
}

impl From<serde_json::Error> for TastyTradeError {
    /// Converts a `serde_json::Error` into a `TastyTradeError`.
    ///
    /// This function maps a serialization/deserialization error from the `serde_json`
    /// crate into a `TastyTradeError::Json` variant.  This allows for consistent error
    /// handling within the `tastytrade` crate, wrapping external errors into the crate's
    /// own error type.
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<DxFeedError> for TastyTradeError {
    /// Converts a `DxFeedError` into a `TastyTradeError`.
    ///
    /// This function maps a `DxFeedError` to the `DxFeed` variant of the `TastyTradeError` enum.  This allows for consistent error handling across the library by representing errors from the DxFeed library within the TastyTrade error handling framework.
    ///
    /// # Arguments
    ///
    /// * `err` - The `DxFeedError` to be converted.
    ///
    /// # Returns
    ///
    /// * A `TastyTradeError` representing the provided `DxFeedError`.
    ///
    /// # Example
    ///
    /// ```
    /// use tastytrade::{DxFeedError, TastyTradeError};
    ///
    /// let dxfeed_error = DxFeedError::CreateConnectionError;
    /// let tastytrade_error = TastyTradeError::from(dxfeed_error);
    ///
    /// assert!(matches!(tastytrade_error, TastyTradeError::DxFeed(_)));
    /// ```
    fn from(err: DxFeedError) -> Self {
        Self::DxFeed(err)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for TastyTradeError {
    /// Converts a `tokio_tungstenite::tungstenite::Error` into a `TastyTradeError`.
    /// This function maps a WebSocket error from the underlying `tungstenite` crate
    /// to a `TastyTradeError::WebSocket` variant, allowing for consistent error
    /// handling throughout the library.
    ///
    /// # Arguments
    ///
    /// * `err` - The `tokio_tungstenite::tungstenite::Error` to be converted.
    ///
    /// # Returns
    ///
    /// * A `TastyTradeError::WebSocket` containing the original `tungstenite` error.
    ///
    /// # Example
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    /// use tokio_tungstenite::tungstenite::Error;
    ///
    /// let ws_error = Error::ConnectionClosed; // Example tungstenite error
    /// let tasty_error = TastyTradeError::from(ws_error);
    ///
    /// assert!(matches!(tasty_error, TastyTradeError::WebSocket(_)));
    /// ```
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(err))
    }
}

impl From<io::Error> for TastyTradeError {
    /// Converts an [`io::Error`] into a [`TastyTradeError`].
    ///
    /// This function maps an [`io::Error`] to the `Io` variant of the
    /// [`TastyTradeError`] enum, allowing for consistent error handling
    /// within the library.  This is typically used in situations where
    /// I/O operations might fail, such as reading from files or network
    /// connections, and those errors need to be propagated up as
    /// [`TastyTradeError`]s.
    ///
    /// # Example
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    /// use std::io;
    ///
    /// let io_error = io::Error::new(io::ErrorKind::Other, "An I/O error occurred");
    /// let tasty_error = TastyTradeError::from(io_error);
    ///
    /// assert!(matches!(tasty_error, TastyTradeError::Io(_)));
    /// ```
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<dxlink::DXLinkError> for TastyTradeError {
    /// Converts a `dxlink::DXLinkError` into a `TastyTradeError`.
    ///
    /// This function maps the various error types from the `dxlink` crate
    /// to the corresponding variants of the `TastyTradeError` enum.  This
    /// allows for consistent error handling throughout the library.
    fn from(err: dxlink::DXLinkError) -> Self {
        match err {
            dxlink::DXLinkError::Authentication(e) => Self::Auth(e),
            dxlink::DXLinkError::Connection(e) => Self::Connection(e),
            dxlink::DXLinkError::WebSocket(e) => {
                // Convert tungstenite 0.26 error to 0.27 error by creating a new error with the same message
                let converted_error = tokio_tungstenite::tungstenite::Error::Io(
                    std::io::Error::other(format!("WebSocket error: {}", e)),
                );
                Self::WebSocket(Box::new(converted_error))
            }
            dxlink::DXLinkError::Serialization(e) => Self::Json(e),
            _ => Self::Streaming(format!("DXLink error: {}", err)),
        }
    }
}

impl TastyTradeError {
    /// Creates a new `TastyTradeError` of the `Auth` variant.
    ///
    /// This function is used to create an error specifically related to authentication issues, such as invalid credentials.
    /// It takes a message string as input, which is converted into a `String` and stored within the `Auth` variant of the `TastyTradeError` enum.
    ///
    /// # Examples
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    ///
    /// let error = TastyTradeError::auth_error("Invalid username or password");
    ///
    /// assert!(matches!(error, TastyTradeError::Auth(_)));
    /// ```
    pub fn auth_error(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }

    /// Creates a new `TastyTradeError` of the `Connection` variant.
    ///
    /// This function is used to create an error specifically related to connection issues, such as network failures or inability to reach the Tastytrade API.
    /// It takes a message string as input, which is converted into a `String` and stored within the `Connection` variant of the `TastyTradeError` enum.
    ///
    /// # Examples
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    ///
    /// let error = TastyTradeError::connection_error("Failed to connect to the server");
    ///
    /// assert!(matches!(error, TastyTradeError::Connection(_)));
    /// ```
    pub fn connection_error(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// Creates a new `TastyTradeError` of the `Streaming` variant.
    ///
    /// This function is used to create an error specifically related to streaming data issues, such as disconnections or data parsing errors. It takes a message string as input, which is converted into a `String` and stored within the `Streaming` variant of the `TastyTradeError` enum.
    ///
    /// # Examples
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    ///
    /// let error = TastyTradeError::streaming_error("Streaming connection lost");
    ///
    /// assert!(matches!(error, TastyTradeError::Streaming(_)));
    /// ```
    pub fn streaming_error(msg: impl Into<String>) -> Self {
        Self::Streaming(msg.into())
    }

    /// Creates a new `TastyTradeError` of the `Unknown` variant.
    ///
    /// This function is used to create an error representing an unknown or unexpected error condition.  It takes a message string as input, which is converted into a `String` and stored within the `Unknown` variant of the `TastyTradeError` enum.
    ///
    /// # Examples
    ///
    /// ```
    /// use tastytrade::TastyTradeError;
    ///
    /// let error = TastyTradeError::unknown_error("Something went wrong");
    ///
    /// assert!(matches!(error, TastyTradeError::Unknown(_)));
    /// ```
    pub fn unknown_error(msg: impl Into<String>) -> Self {
        Self::Unknown(msg.into())
    }
}
