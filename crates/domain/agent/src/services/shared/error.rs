//! Service-layer error type, distinct from the crate-public [`AgentError`]:
//! it models failures internal to runtime services and converts into
//! `AgentError` at the crate boundary.
//!
//! [`AgentError`]: crate::error::AgentError
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentServiceError {
    #[error("database operation failed: {0}")]
    Database(String),

    #[error("repository operation failed: {0}")]
    Repository(String),

    #[error("network request failed: {0}")]
    Network(String),

    #[error("authentication failed: {0}")]
    Authentication(String),

    #[error("authorization failed for resource: {0}")]
    Authorization(String),

    #[error("validation failed: {0}: {1}")]
    Validation(String, String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("operation timed out after {0}ms")]
    Timeout(u64),

    #[error("configuration error: {0}: {1}")]
    Configuration(String, String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("logging error: {0}")]
    Logging(String),

    #[error("capacity exceeded: {0}")]
    Capacity(String),
}

impl From<std::io::Error> for AgentServiceError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(format!("io: {err}"))
    }
}

impl From<sqlx::Error> for AgentServiceError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl From<crate::repository::RepositoryError> for AgentServiceError {
    fn from(err: crate::repository::RepositoryError) -> Self {
        Self::Repository(err.to_string())
    }
}

impl From<systemprompt_database::RepositoryError> for AgentServiceError {
    fn from(err: systemprompt_database::RepositoryError) -> Self {
        Self::Repository(err.to_string())
    }
}

impl From<crate::error::AgentError> for AgentServiceError {
    fn from(err: crate::error::AgentError) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<systemprompt_models::errors::ProviderError> for AgentServiceError {
    fn from(err: systemprompt_models::errors::ProviderError) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<reqwest::Error> for AgentServiceError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(
            err.url()
                .map_or_else(|| "unknown".to_owned(), ToString::to_string),
        )
    }
}

pub type Result<T> = std::result::Result<T, AgentServiceError>;
