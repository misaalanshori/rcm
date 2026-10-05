use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CoreError {
    #[error("Invalid remote name '{0}': must not be empty and cannot contain ':' or '\\'")]
    InvalidRemoteName(String),

    #[error("Invalid mount target: {0}")]
    InvalidMountTarget(String),

    #[error("Invalid drive letter '{0}': must be an ASCII alphabetic character (A-Z)")]
    InvalidDriveLetter(char),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Resource not found: {0}")]
    NotFound(String),
}
