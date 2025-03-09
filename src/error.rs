use serde::Deserialize;
use std::error::Error;
use std::fmt::{self, Display};
use std::io;

#[derive(Debug)]
pub enum DxFeedError {
    CreateConnectionError,
}

impl Display for DxFeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DxFeed error: {:?}", self)
    }
}

impl Error for DxFeedError {}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: Option<String>,
    pub message: String,
    pub errors: Option<Vec<InnerApiError>>,
}

#[derive(Debug, Deserialize)]
pub struct InnerApiError {
    pub code: Option<String>,
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error {:?}: {}", self.code, self.message)
    }
}

impl Error for ApiError {}

#[derive(Debug)]
pub enum TastyTradeError {
    Api(ApiError),
    Http(reqwest::Error),
    Json(serde_json::Error),
    DxFeed(DxFeedError),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Io(io::Error),
    Auth(String),
    Connection(String),
    Streaming(String),
    Unknown(String),
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
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Api(err) => Some(err),
            Self::Http(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::DxFeed(err) => Some(err),
            Self::WebSocket(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::Auth(_) => None,
            Self::Connection(_) => None,
            Self::Streaming(_) => None,
            Self::Unknown(_) => None,
            Self::ConfigError(_) => None,
        }
    }
}

// Conversion implementations (replacing the #[from] attributes)
impl From<ApiError> for TastyTradeError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

impl From<reqwest::Error> for TastyTradeError {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err)
    }
}

impl From<serde_json::Error> for TastyTradeError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<DxFeedError> for TastyTradeError {
    fn from(err: DxFeedError) -> Self {
        Self::DxFeed(err)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for TastyTradeError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(err)
    }
}

impl From<io::Error> for TastyTradeError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<dxlink::DXLinkError> for TastyTradeError {
    fn from(err: dxlink::DXLinkError) -> Self {
        match err {
            dxlink::DXLinkError::Authentication(e) => Self::Auth(e),
            dxlink::DXLinkError::Connection(e) => Self::Connection(e),
            dxlink::DXLinkError::WebSocket(e) => Self::WebSocket(e),
            dxlink::DXLinkError::Serialization(e) => Self::Json(e),
            _ => Self::Streaming(format!("DXLink error: {}", err)),
        }
    }
}

impl TastyTradeError {
    pub fn auth_error(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }

    pub fn connection_error(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    pub fn streaming_error(msg: impl Into<String>) -> Self {
        Self::Streaming(msg.into())
    }

    pub fn unknown_error(msg: impl Into<String>) -> Self {
        Self::Unknown(msg.into())
    }
}
