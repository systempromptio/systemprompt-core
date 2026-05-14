//! Public error type for provider trait contracts.
//!
//! [`ProviderError`] is the concrete error returned by every provider trait
//! that does not have a domain-specific error of its own (LLM and tool
//! providers carry their own typed errors — see [`crate::llm`] and
//! [`crate::tool`]).
//!
//! Downstream provider crates that implement these traits convert any
//! third-party error at the boundary with
//! `.map_err(|e| ProviderError::Internal(e.to_string()))`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider configuration error: {0}")]
    Configuration(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Render failed: {0}")]
    RenderFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal provider error: {0}")]
    Internal(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;
