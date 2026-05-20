//! Typed error boundary for the runtime crate.
//!
//! All public APIs of `systemprompt-runtime` return [`RuntimeResult<T>`]
//! (i.e. `Result<T, RuntimeError>`). [`RuntimeError`] composes the typed
//! errors of upstream layers (config, database, events, files, users,
//! extensions) via `#[from]` so callers can pattern-match on the original
//! cause without losing fidelity.
//!
//! Third-party errors without a `#[from]` adapter are stringified into
//! the [`RuntimeError::Internal`] variant at the call site so the lossy
//! conversion is visible.

use systemprompt_analytics::AnalyticsError;
use systemprompt_config::{ConfigError as ProfileConfigError, ProfileBootstrapError};
use systemprompt_database::RepositoryError;
use systemprompt_extension::LoaderError;
use systemprompt_files::FilesError;
use systemprompt_models::errors::ConfigError as ModelConfigError;
use systemprompt_models::paths::PathError;
use systemprompt_users::UserError;
use thiserror::Error;

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Profile(#[from] ProfileConfigError),

    #[error(transparent)]
    ProfileBootstrap(#[from] ProfileBootstrapError),

    #[error(transparent)]
    Config(#[from] ModelConfigError),

    #[error(transparent)]
    Paths(#[from] PathError),

    #[error(transparent)]
    Files(#[from] FilesError),

    #[error(transparent)]
    Users(#[from] UserError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error(transparent)]
    Analytics(#[from] AnalyticsError),

    #[error(transparent)]
    Loader(#[from] LoaderError),

    #[error(
        "Configured system admin '{username}' was not found in the users table. Run `systemprompt \
         admin bootstrap` first."
    )]
    SystemAdminNotFound { username: String },

    #[error(
        "Configured system admin '{username}' exists but is not active. Re-activate the user \
         before starting the platform."
    )]
    SystemAdminInactive { username: String },

    #[error(
        "Configured system admin '{username}' exists but does not carry the 'admin' role. Grant \
         the role before starting the platform."
    )]
    SystemAdminMissingRole { username: String },

    #[error("UserService unavailable during AppContext bootstrap; system admin cannot be resolved")]
    SystemAdminUserServiceUnavailable,

    #[error("DATABASE_URL is empty")]
    EmptyDatabaseUrl,

    #[error("Database not found at '{path}'. Run setup first")]
    DatabaseNotFound { path: String },

    #[error("Database path '{path}' exists but is not a file")]
    DatabaseNotFile { path: String },

    #[error("internal: {0}")]
    Internal(String),
}
