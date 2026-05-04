//! Error types for the systemprompt-sync crate.
//!
//! [`SyncError`] is the unified `thiserror` enum returned by every public
//! function and method in this crate. Upstream errors (`std::io`, `reqwest`,
//! `serde_json`, `sqlx`, `zip`) are auto-converted via `#[from]`; everything
//! else is funnelled through a typed variant or [`SyncError::Other`].
//!
//! [`SyncResult<T>`](SyncResult) is the canonical `Result` alias.

use thiserror::Error;

/// Errors returned by the systemprompt-sync crate.
#[derive(Debug, Error)]
pub enum SyncError {
    /// Cloud API returned a non-2xx HTTP status.
    #[error("API error {status}: {message}")]
    ApiError {
        /// HTTP status code returned by the cloud API.
        status: u16,
        /// Server-provided error message.
        message: String,
    },

    /// API call returned 401 — the local credentials are missing or stale.
    #[error("Unauthorized - run 'systemprompt cloud login'")]
    Unauthorized,

    /// The active tenant has no app associated with it (sync target unknown).
    #[error("Tenant has no associated app")]
    TenantNoApp,

    /// The CLI was invoked from a directory that does not look like a
    /// systemprompt project root (no `infrastructure/` directory).
    #[error("Must run from project root (with infrastructure/ directory)")]
    NotProjectRoot,

    /// An external command exited with a non-zero status.
    #[error("Command failed: {command}")]
    CommandFailed {
        /// Command line that failed.
        command: String,
    },

    /// `Command::spawn` failed before the child process could start.
    #[error("Failed to spawn `{command}`: {source}")]
    CommandSpawnFailed {
        /// Command line that failed to spawn.
        command: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Could not open a file for reading.
    #[error("Failed to open file {path}: {source}")]
    FileOpenFailed {
        /// Path that failed to open.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// `docker login` failed (typically wrong registry credentials).
    #[error("Docker login failed")]
    DockerLoginFailed,

    /// Could not determine the current git commit SHA.
    #[error("Git SHA unavailable")]
    GitShaUnavailable,

    /// A required configuration value was missing.
    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    /// A bulk import partially succeeded — `completed` of `total` items were
    /// imported before a failure stopped the run.
    #[error("Partial import failure after {completed}/{total} items: {message}")]
    PartialImport {
        /// Number of items that were imported before the failure.
        completed: usize,
        /// Total items the run was attempting to import.
        total: usize,
        /// Display string of the underlying error.
        message: String,
    },

    /// A tarball entry escaped the extraction root (path traversal).
    #[error("Unsafe tarball entry rejected: {0}")]
    TarballUnsafe(String),

    /// Filesystem I/O failure.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request failure (`reqwest`).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parse or serialisation failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML parse or serialisation failure.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Database driver failure.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// `Path::strip_prefix` failure during file enumeration.
    #[error("Path error: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),

    /// Zip archive read/write failure.
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Validation failure or malformed input that does not fit any other
    /// variant.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Catch-all for upstream errors without a dedicated `#[from]` variant.
    /// Prefer typed variants in new code.
    #[error("{0}")]
    Other(String),
}

impl SyncError {
    /// Build a [`SyncError::Other`] from any [`Display`] value.
    pub fn other(cause: impl std::fmt::Display) -> Self {
        Self::Other(cause.to_string())
    }

    /// Build a [`SyncError::InvalidInput`] from any [`Display`] value.
    pub fn invalid_input(cause: impl std::fmt::Display) -> Self {
        Self::InvalidInput(cause.to_string())
    }

    /// Whether the error is a transient failure worth retrying.
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Http(_))
            || matches!(
                self,
                Self::ApiError { status, .. }
                    if *status == 502 || *status == 503 || *status == 504 || *status == 429
            )
    }
}

/// Canonical `Result` alias for the sync crate.
pub type SyncResult<T> = Result<T, SyncError>;
