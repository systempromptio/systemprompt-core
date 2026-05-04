//! Typed error returned by [`crate::llm::LlmProvider`] implementations.

use thiserror::Error;

/// Failure modes a chat-completion call may surface.
#[derive(Debug, Error)]
pub enum LlmProviderError {
    /// The requested model is not in the provider's supported set.
    #[error("Model '{0}' not supported")]
    ModelNotSupported(String),

    /// The named provider is not configured / wired up.
    #[error("Provider '{0}' not available")]
    ProviderNotAvailable(String),

    /// The provider returned a rate-limit signal.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// Authentication with the upstream provider failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// The request did not satisfy the provider's contract.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// The model failed to produce a valid completion.
    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    /// Catch-all for provider-internal failures.
    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

impl From<anyhow::Error> for LlmProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

/// Convenience alias for `Result<T, LlmProviderError>`.
pub type LlmProviderResult<T> = Result<T, LlmProviderError>;
