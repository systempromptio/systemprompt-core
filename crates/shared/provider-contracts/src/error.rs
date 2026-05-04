//! Public error type for provider trait contracts.
//!
//! [`ProviderError`] is the concrete error returned by every provider trait
//! that does not have a domain-specific error of its own (LLM and tool
//! providers carry their own typed errors — see [`crate::llm`] and
//! [`crate::tool`]).
//!
//! Downstream provider crates that implement these traits typically use
//! `anyhow::Error` internally; the `#[from]` impl lets them propagate with
//! `?` while still presenting a typed error at the public boundary.

use thiserror::Error;

/// Errors returned by provider trait implementations.
///
/// Variants are intentionally coarse — provider crates surface their own
/// internal detail through [`ProviderError::Internal`] or via the
/// `#[from] anyhow::Error` conversion.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// A required configuration value or asset was missing.
    #[error("Provider configuration error: {0}")]
    Configuration(String),

    /// The provider could not locate the requested resource.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// The provider received input that did not satisfy its contract.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// The provider failed to render, render-prepare, or transform content.
    #[error("Render failed: {0}")]
    RenderFailed(String),

    /// A downstream I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A YAML parse or serialization step failed.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// A JSON parse or serialization step failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Catch-all for provider-internal failures.
    #[error("Internal provider error: {0}")]
    Internal(#[source] anyhow::Error),
}

impl From<anyhow::Error> for ProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

/// Convenience alias for `Result<T, ProviderError>`.
pub type ProviderResult<T> = Result<T, ProviderError>;
