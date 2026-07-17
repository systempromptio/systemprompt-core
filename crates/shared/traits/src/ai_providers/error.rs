//! AI provider error type and result alias.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
