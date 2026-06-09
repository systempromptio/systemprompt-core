//! AI provider error type and result alias.

pub type AiProviderResult<T> = Result<T, AiProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiProviderError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Storage error: {message}")]
    StorageError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Internal error: {0}")]
    Internal(String),
}
