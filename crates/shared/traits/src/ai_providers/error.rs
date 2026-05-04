//! AI provider error type and result alias.

pub type AiProviderResult<T> = Result<T, AiProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiProviderError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AiProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
