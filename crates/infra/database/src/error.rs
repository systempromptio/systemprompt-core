//! Typed error boundary for the database crate.
//!
//! `RepositoryError` is the canonical error returned from public, non-trait
//! signatures (constructors, repositories, lifecycle helpers, validation
//! helpers). It composes `sqlx::Error` and `serde_json::Error` via `#[from]`
//! and accepts upstream `anyhow::Error` for the dyn-safe trait surface that
//! still propagates dynamic errors.

use thiserror::Error;

/// Canonical error type for `systemprompt-database` public APIs.
///
/// Every variant carries enough context to be logged and reported without
/// leaking SQL fragments or sensitive parameter values.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Requested entity (row, table, configuration record) does not exist.
    #[error("Entity not found: {0}")]
    NotFound(String),

    /// A database-level constraint (unique, foreign key, check, …) was
    /// violated by the requested operation.
    #[error("Constraint violation: {0}")]
    Constraint(String),

    /// Underlying `SQLx` error — connection failures, protocol errors, decode
    /// errors, etc.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Failure to serialise/deserialise JSON payloads stored in or read from
    /// the database.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Caller supplied an invalid argument (empty identifier, out-of-range
    /// value, malformed parameter, …).
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Catch-all for repository-internal failures that do not fit any other
    /// variant. Prefer one of the more specific variants when possible.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Convenience alias used by all public, non-trait database APIs.
pub type DatabaseResult<T> = Result<T, RepositoryError>;

impl RepositoryError {
    /// Construct a [`RepositoryError::NotFound`] from any displayable id.
    pub fn not_found<T: std::fmt::Display>(id: T) -> Self {
        Self::NotFound(id.to_string())
    }

    /// Construct a [`RepositoryError::Constraint`] from any string-like
    /// message.
    pub fn constraint<T: Into<String>>(message: T) -> Self {
        Self::Constraint(message.into())
    }

    /// Construct a [`RepositoryError::InvalidArgument`] from any string-like
    /// message.
    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }

    /// Construct a [`RepositoryError::Internal`] from any string-like message.
    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self::Internal(message.into())
    }

    /// Returns true if this is a [`RepositoryError::NotFound`] variant.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    /// Returns true if this is a [`RepositoryError::Constraint`] variant.
    #[must_use]
    pub const fn is_constraint(&self) -> bool {
        matches!(self, Self::Constraint(_))
    }
}

impl From<anyhow::Error> for RepositoryError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
