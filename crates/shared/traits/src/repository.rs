//! Generic repository error type used by domain crates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(Box<dyn std::error::Error + Send + Sync>),

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("{0}")]
    Internal(String),
}

impl RepositoryError {
    pub fn database(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Database(Box::new(err))
    }
}
