//! Typed error returned by [`crate::llm::LlmProvider`] implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmProviderError {
    #[error("Model '{0}' not supported")]
    ModelNotSupported(String),

    #[error("Provider '{0}' not available")]
    ProviderNotAvailable(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type LlmProviderResult<T> = Result<T, LlmProviderError>;
