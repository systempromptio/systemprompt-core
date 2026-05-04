//! Typed error surface for the content crate.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContentError {
    #[error("database must be PostgreSQL")]
    DatabaseNotPostgres,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("content not found: {0}")]
    ContentNotFound(String),

    #[error("link not found: {0}")]
    LinkNotFound(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("service error: {0}")]
    Service(String),
}

pub type ContentResult<T> = Result<T, ContentError>;
