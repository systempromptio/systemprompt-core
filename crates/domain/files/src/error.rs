//! Typed error surface for the `systemprompt-files` crate.

use thiserror::Error;

/// Errors produced by file storage, metadata, validation, and ingestion code
/// paths.
#[derive(Debug, Error)]
pub enum FilesError {
    /// File not found in the metadata store or on disk.
    #[error("file not found: {0}")]
    NotFound(String),

    /// Failure interacting with the underlying storage backend.
    #[error("storage error: {0}")]
    Storage(String),

    /// `SQLx` error originating from a file metadata repository call.
    #[error("repository error: {0}")]
    Repository(#[from] sqlx::Error),

    /// I/O failure while reading or writing a file on disk.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Validation failure — e.g. unsupported MIME, oversize upload, bad path.
    #[error("validation error: {0}")]
    Validation(String),

    /// Configuration failure — `FilesConfig::init` not called, missing storage
    /// path, etc.
    #[error("config error: {0}")]
    Config(String),

    /// Failure deserializing structured metadata stored alongside a file.
    #[error("metadata serde error: {0}")]
    Metadata(#[from] serde_json::Error),

    /// Failure parsing the on-disk YAML configuration file.
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Escape hatch for callers that still need `anyhow` interop.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience alias for `Result<T, FilesError>`.
pub type FilesResult<T> = Result<T, FilesError>;
