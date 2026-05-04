//! Typed error returned by [`crate::tool::ToolProvider`] implementations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolProviderError {
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    #[error("Failed to connect to service '{service}': {message}")]
    ConnectionFailed { service: String, message: String },

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ToolProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

pub type ToolProviderResult<T> = Result<T, ToolProviderError>;
