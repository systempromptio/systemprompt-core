//! AI provider error type and result alias.

/// Result alias for AI provider operations.
pub type AiProviderResult<T> = Result<T, AiProviderError>;

/// Errors returned by AI providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiProviderError {
    /// The requested AI-generated file does not exist.
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// The requested AI session does not exist.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// The configured storage backend reported a failure.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// The provider is misconfigured.
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Catch-all for unexpected failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AiProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
