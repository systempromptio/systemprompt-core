//! Public error type for provider trait contracts.
//!
//! [`ProviderError`] is the concrete error returned by every provider trait
//! that does not have a domain-specific error of its own (LLM and tool
//! providers carry their own typed errors — see [`crate::llm`] and
//! [`crate::tool`]).
//!
//! Downstream provider crates that implement these traits keep a
//! third-party error as the variant's source, e.g.
//! `.map_err(|e| ProviderError::Internal(Box::new(e)))`, rather than its text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use crate::dependencies::MissingDependency;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider configuration error: {0}")]
    Configuration(String),

    #[error("Provider configuration error: {context}: {source}")]
    ConfigurationLoad {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("invalid job parameter {key}={value}: {source}")]
    InvalidParameter {
        key: String,
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error(transparent)]
    MissingDependency(#[from] MissingDependency),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Render failed: {0}")]
    RenderFailed(String),

    #[error("Render failed: {context}: {source}")]
    Rendering {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal provider error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub type ProviderResult<T> = Result<T, ProviderError>;
