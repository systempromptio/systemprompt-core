use thiserror::Error;

use super::FileValidationError;

#[derive(Debug, Error)]
pub enum FileUploadError {
    #[error("File persistence is disabled")]
    PersistenceDisabled,

    #[error("Validation failed: {0}")]
    Validation(#[from] FileValidationError),

    #[error("Failed to decode base64: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Base64 input too large: encoded size {encoded_size} bytes exceeds limit")]
    Base64TooLarge { encoded_size: usize },

    #[error("Path validation failed: {0}")]
    PathValidation(String),
}
