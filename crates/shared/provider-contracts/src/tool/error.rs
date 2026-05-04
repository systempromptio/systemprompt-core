//! Typed error returned by [`crate::tool::ToolProvider`] implementations.

use thiserror::Error;

/// Failure modes a [`crate::tool::ToolProvider`] call may surface.
#[derive(Debug, Error)]
pub enum ToolProviderError {
    /// The requested tool is not registered with this provider.
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    /// The named backing service is not registered with this provider.
    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    /// The provider could not reach the backing service.
    #[error("Failed to connect to service '{service}': {message}")]
    ConnectionFailed {
        /// Service identifier that failed to connect.
        service: String,
        /// Human-readable failure detail.
        message: String,
    },

    /// The tool ran but returned an execution-time error.
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    /// The caller is not authorized to invoke the tool.
    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    /// The provider was misconfigured (missing credentials, bad URL, ...).
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Catch-all for provider-internal failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ToolProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Convenience alias for `Result<T, ToolProviderError>`.
pub type ToolProviderResult<T> = Result<T, ToolProviderError>;
