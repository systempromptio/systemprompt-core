//! Typed error boundary for the database crate.
//!
//! `RepositoryError` is the canonical error returned from the crate's
//! database-facing public signatures, including the dyn-safe
//! `DatabaseProvider` / `DatabaseTransaction` trait surfaces. It composes
//! `sqlx::Error` and `serde_json::Error` via `#[from]`; runtime invariant
//! failures are routed through `RepositoryError::InvalidState`.
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

    #[error("SQL could not be split into statements: {0}")]
    SqlSplit(#[source] pg_query::Error),

    #[error("Failed to execute SQL statement: {statement}")]
    Statement {
        statement: String,
        #[source]
        source: Box<Self>,
    },

    #[error("Failed to establish database connection")]
    Connection(#[source] Box<Self>),

    #[error("Failed to read SQL file {path}")]
    SqlFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub type DatabaseResult<T> = Result<T, RepositoryError>;

impl RepositoryError {
    pub fn not_found<T: std::fmt::Display>(id: T) -> Self {
        Self::NotFound(id.to_string())
    }

    pub fn is_serialization_failure(&self) -> bool {
        // Why: Postgres aborts one side of a serialization conflict (40001) or a
        // deadlock (40P01) and documents both as "retry the transaction".
        match self {
            Self::Database(sqlx_error) => sqlx_error.as_database_error().is_some_and(|db_error| {
                let code = db_error.code().map(|c| c.to_string());
                matches!(code.as_deref(), Some("40001" | "40P01"))
            }),
            _ => false,
        }
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
        Self::database(err)
    }
}
