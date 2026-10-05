use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RcRawError {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcErrorKind {
    Transient,
    Permanent,
    Auth,
    NotFound,
    ConnectionFailed,
}

#[derive(Debug, Error)]
pub enum RcError {
    #[error("HTTP error ({status}): {message}")]
    Http {
        status: u16,
        message: String,
        raw: Option<Box<RcRawError>>,
    },

    #[error("Network/connection failed: {0}")]
    Connection(String),

    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API error from rclone: {message} (status: {status:?}, path: {path:?})")]
    Api {
        status: Option<u16>,
        message: String,
        path: Option<String>,
        raw: Box<RcRawError>,
    },

    #[error("Wizard error: {0}")]
    Wizard(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

impl RcError {
    pub fn kind(&self) -> RcErrorKind {
        match self {
            RcError::Http { status, .. } => {
                if *status == 401 || *status == 403 {
                    RcErrorKind::Auth
                } else if *status == 404 {
                    RcErrorKind::NotFound
                } else if *status >= 500 {
                    RcErrorKind::Transient
                } else {
                    RcErrorKind::Permanent
                }
            }
            RcError::Api { status, raw, .. } => {
                if let Some(s) = status {
                    if *s == 401 || *s == 403 {
                        return RcErrorKind::Auth;
                    }
                    if *s == 404 {
                        return RcErrorKind::NotFound;
                    }
                }
                let err_lower = raw.error.to_lowercase();
                if err_lower.contains("connection refused")
                    || err_lower.contains("timeout")
                    || err_lower.contains("temporary")
                {
                    RcErrorKind::Transient
                } else {
                    RcErrorKind::Permanent
                }
            }
            RcError::Connection(_) => RcErrorKind::ConnectionFailed,
            RcError::Timeout(_) => RcErrorKind::Transient,
            _ => RcErrorKind::Permanent,
        }
    }
}
