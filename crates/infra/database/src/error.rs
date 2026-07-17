//! Typed error boundary for the database crate.
//!
//! `RepositoryError` is the canonical error returned from the crate's
//! database-facing public signatures, including the dyn-safe
//! `DatabaseProvider` / `DatabaseTransaction` trait surfaces. It composes
//! `sqlx::Error` and `serde_json::Error` via `#[from]`; runtime invariant
//! failures are routed through `RepositoryError::InvalidState`. The
//! filesystem-only [`crate::squash_baseline`] module carries its own error
//! type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Failed to execute query")]
    QueryExecution(#[source] Box<Self>),
}

pub type DatabaseResult<T> = Result<T, RepositoryError>;

impl RepositoryError {
    pub fn not_found<T: std::fmt::Display>(id: T) -> Self {
        Self::NotFound(id.to_string())
    }

    pub fn constraint<T: Into<String>>(message: T) -> Self {
        Self::Constraint(message.into())
    }

    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self::Internal(message.into())
    }

    pub fn invalid_state<T: Into<String>>(message: T) -> Self {
        Self::InvalidState(message.into())
    }

    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    #[must_use]
    pub const fn is_constraint(&self) -> bool {
        matches!(self, Self::Constraint(_))
    }
}

impl From<RepositoryError> for systemprompt_traits::RepositoryError {
    fn from(err: RepositoryError) -> Self {
        Self::Database(Box::new(err))
    }
}
