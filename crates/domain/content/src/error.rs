//! Typed error surface for the content crate.

use thiserror::Error;

/// Errors produced by content ingestion, repository, search, and link-tracking
/// code paths.
#[derive(Error, Debug)]
pub enum ContentError {
    /// Underlying database backend is not `PostgreSQL`.
    #[error("database must be PostgreSQL")]
    DatabaseNotPostgres,

    /// `SQLx` error bubbled up from a repository call.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Content row was not found for the supplied identifier.
    #[error("content not found: {0}")]
    ContentNotFound(String),

    /// Tracked link row was not found for the supplied short code or id.
    #[error("link not found: {0}")]
    LinkNotFound(String),

    /// Request parameters were structurally invalid.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// JSON serialisation/deserialisation failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Domain validation failed (e.g. metadata missing required fields).
    #[error("validation error: {0}")]
    Validation(String),

    /// Markdown front-matter or content body could not be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// I/O failure while reading a content file.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parse failure (front-matter, source manifests).
    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Failure constructing a downstream service or repository.
    #[error("service error: {0}")]
    Service(String),
}

/// Convenience alias for `Result<T, ContentError>`.
pub type ContentResult<T> = Result<T, ContentError>;
