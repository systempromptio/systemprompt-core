//! Typed error boundary for the runtime crate.
//!
//! All public APIs of `systemprompt-runtime` return [`RuntimeResult<T>`]
//! (i.e. `Result<T, RuntimeError>`). [`RuntimeError`] composes the typed
//! errors of upstream layers (config, database, events, files, users,
//! extensions) via `#[from]` so callers can pattern-match on the original
//! cause without losing fidelity.
//!
//! Calls into upstream crates that still return `anyhow::Result` (for
//! example schema/seed installation and database connectivity probes)
//! are absorbed into the [`RuntimeError::Other`] variant.

use systemprompt_analytics::AnalyticsError;
use systemprompt_config::{ConfigError as ProfileConfigError, ProfileBootstrapError};
use systemprompt_database::RepositoryError;
use systemprompt_extension::LoaderError;
use systemprompt_files::FilesError;
use systemprompt_models::errors::ConfigError as ModelConfigError;
use systemprompt_models::paths::PathError;
use systemprompt_users::UserError;
use thiserror::Error;

/// Result alias for runtime operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Top-level error for the `systemprompt-runtime` public API.
///
/// Variants compose upstream typed errors via `#[from]`. The
/// [`RuntimeError::Other`] variant exists for upstream APIs that still
/// return `anyhow::Error`; once those are typed it can be removed.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Profile / secrets / credentials bootstrap failure (composite).
    #[error(transparent)]
    Profile(#[from] ProfileConfigError),

    /// Profile bootstrap accessor failure (uninitialised).
    #[error(transparent)]
    ProfileBootstrap(#[from] ProfileBootstrapError),

    /// Static `Config` accessor failure (config not initialised).
    #[error(transparent)]
    Config(#[from] ModelConfigError),

    /// `AppPaths` derivation failed (missing or invalid path entries).
    #[error(transparent)]
    Paths(#[from] PathError),

    /// Files subsystem initialisation or lookup failure.
    #[error(transparent)]
    Files(#[from] FilesError),

    /// User service initialisation or query failure.
    #[error(transparent)]
    Users(#[from] UserError),

    /// Database repository / pool failure.
    #[error(transparent)]
    Repository(#[from] RepositoryError),

    /// Analytics subsystem initialisation or query failure.
    #[error(transparent)]
    Analytics(#[from] AnalyticsError),

    /// Extension registry validation failure.
    #[error(transparent)]
    Loader(#[from] LoaderError),

    /// Database URL is empty.
    #[error("DATABASE_URL is empty")]
    EmptyDatabaseUrl,

    /// `SQLite` database file is missing on disk.
    #[error("Database not found at '{path}'. Run setup first")]
    DatabaseNotFound {
        /// The path that was probed.
        path: String,
    },

    /// `SQLite` database path exists but is not a regular file.
    #[error("Database path '{path}' exists but is not a file")]
    DatabaseNotFile {
        /// The offending path.
        path: String,
    },

    /// Catch-all for upstream APIs that still return `anyhow::Error`.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
