//! Typed error hierarchy for the `systemprompt-agent` crate.
//!
//! Public APIs return concrete `thiserror`-derived enums instead of
//! `anyhow::Error` so that downstream callers can match on error variants
//! without string parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_traits::{BoxedSource, MetadataValidationError, RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid tool response schema: expected {expected}, found keys: {actual_keys:?}")]
    InvalidSchema {
        expected: &'static str,
        actual_keys: Vec<String>,
        #[source]
        source: serde_json::Error,
    },

    #[error("Invalid artifact context id: {0}")]
    InvalidContextId(#[source] IdValidationError),

    #[error("Metadata validation error: {0}")]
    MetadataValidation(#[from] MetadataValidationError),

    #[error("Transform error: {0}")]
    Transform(String),
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Artifact error: {0}")]
    Artifact(#[from] ArtifactError),

    #[error("repository: {0}")]
    Repository(#[from] RepositoryError),

    #[error("config: {0}")]
    Config(String),

    #[error("config: {context}: {source}")]
    InvalidConfig {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("validation: {0}")]
    Validation(String),

    #[error("No available ports in range {min}-{max}")]
    NoAvailablePort { min: u16, max: u16 },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("services config: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),
}

impl AgentError {
    pub fn invalid_config<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::InvalidConfig {
            context: context.into(),
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for AgentError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(err.into())
    }
}

pub type AgentResult<T> = Result<T, AgentError>;
