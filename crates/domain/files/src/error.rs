//! Typed error surface for the `systemprompt-files` crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilesError {
    #[error("file not found: {0}")]
    NotFound(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("repository error: {0}")]
    Repository(#[from] sqlx::Error),

    #[error("database repository error: {0}")]
    DatabaseRepository(#[from] systemprompt_database::RepositoryError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("metadata serde error: {0}")]
    Metadata(#[from] serde_json::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type FilesResult<T> = Result<T, FilesError>;
