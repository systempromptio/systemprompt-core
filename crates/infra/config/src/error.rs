//! Public error type for `systemprompt-config`.
//!
//! All public APIs of this crate return [`ConfigError`] (or
//! [`ConfigResult<T>`]) instead of `anyhow::Error`. The enum is
//! `#[non_exhaustive]` so additional variants can be added in patch
//! releases without breaking downstream code that performs exhaustive
//! matching only on the documented variants.
//!
//! Upstream errors are composed via `#[from]` so callers can use `?`
//! transparently from `std::io`, `serde_json`, `serde_yaml`, and
//! `regex` operations performed inside the bootstrap and validator
//! pipelines.

use std::path::PathBuf;

use systemprompt_models::errors::SecretsError;
use systemprompt_models::profile::{GatewayProfileError, ProfileError};

use crate::bootstrap::{ProfileBootstrapError, SecretsBootstrapError};
use crate::services::ConfigValidationError;

/// Public result alias used by every public function in this crate.
pub type ConfigResult<T> = Result<T, ConfigError>;

/// Public error returned by every fallible public API in
/// `systemprompt-config`.
///
/// Composes the more specific bootstrap, profile, and validation
/// errors into a single typed surface so the entry layer (CLI/API)
/// can match on the kind of failure when emitting diagnostics.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The global [`systemprompt_models::Config`] cell has already been
    /// installed.
    #[error("Config already initialized")]
    AlreadyInitialized,

    /// Profile bootstrap returned an error during initialization or
    /// lookup.
    #[error(transparent)]
    Profile(#[from] ProfileBootstrapError),

    /// Secrets bootstrap returned an error during load or validation.
    #[error(transparent)]
    Secrets(#[from] SecretsBootstrapError),

    /// Profile YAML parsing or validation failed.
    #[error(transparent)]
    ProfileParse(#[from] ProfileError),

    /// Profile gateway catalog parsing or validation failed.
    #[error(transparent)]
    Gateway(#[from] GatewayProfileError),

    /// Schema validation of a generated YAML config failed.
    #[error(transparent)]
    SchemaValidation(#[from] ConfigValidationError),

    /// Secrets document parsing or validation failed.
    #[error(transparent)]
    SecretsParse(#[from] SecretsError),

    /// I/O error while reading or writing config-related files.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization of config-adjacent structures failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// YAML (de)serialization of a profile or environment file failed.
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    /// Regex compilation for variable substitution failed.
    #[error(transparent)]
    Regex(#[from] regex::Error),

    /// A required path field is missing from the active profile.
    #[error("Missing required path: paths.{field}")]
    MissingProfilePath {
        /// Name of the missing path field.
        field: String,
    },

    /// A profile path could not be canonicalized to an absolute path.
    #[error("Failed to canonicalize {name} path: {source}")]
    CanonicalizePath {
        /// Logical name of the path being canonicalized.
        name: String,
        /// Underlying I/O error from `fs::canonicalize`.
        #[source]
        source: std::io::Error,
    },

    /// A YAML config file referenced from the profile could not be
    /// read.
    #[error("Profile path '{field}' cannot be read: {path}")]
    ReadProfilePath {
        /// Logical field name (e.g. `web_config`).
        field: String,
        /// Path that failed to read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A YAML config file referenced from the profile contained
    /// invalid YAML syntax.
    #[error("Profile path '{field}' has invalid YAML: {path}")]
    InvalidProfileYaml {
        /// Logical field name (e.g. `content_config`).
        field: String,
        /// Path that failed to parse.
        path: PathBuf,
        /// Underlying YAML error.
        #[source]
        source: serde_yaml::Error,
    },

    /// `validate_profile_paths` reported one or more issues.
    #[error("Profile path validation failed: {message}")]
    ProfilePathReport {
        /// Human-readable summary of all validation errors.
        message: String,
    },

    /// `database.type` in the profile is not one of the supported
    /// engines.
    #[error("Unsupported database type '{db_type}'. Only 'postgres' is supported.")]
    UnsupportedDatabaseType {
        /// The raw `database.type` string from the profile.
        db_type: String,
    },

    /// The configured `database.url` is not a valid `PostgreSQL` URL.
    #[error("Invalid database URL: {message}")]
    InvalidDatabaseUrl {
        /// Human-readable description of why the URL was rejected.
        message: String,
    },

    /// Variable substitution could not converge in the configured
    /// number of passes.
    #[error("Failed to resolve variables after {passes} passes: {unresolved}")]
    UnresolvedVariables {
        /// Maximum number of substitution passes attempted.
        passes: usize,
        /// Newline-separated list of `KEY = value` pairs still
        /// containing `${...}` markers.
        unresolved: String,
    },

    /// One or more environment-specific configuration validators
    /// reported errors.
    #[error("{count} validation error(s)")]
    ValidationErrors {
        /// Number of accumulated validation errors.
        count: usize,
    },

    /// `infrastructure/environments/<env>/config.yaml` (or `base.yaml`)
    /// is missing on disk.
    #[error("Required config file missing: {path}")]
    EnvironmentConfigMissing {
        /// Absolute path of the missing file.
        path: PathBuf,
    },

    /// A regex capture group expected by the variable resolver was
    /// missing.
    #[error("Regex capture group {index} missing")]
    MissingCaptureGroup {
        /// Group index that was expected to be present.
        index: usize,
    },

    /// Free-form configuration error with a context message.
    #[error("{message}")]
    Other {
        /// Human-readable description of the problem.
        message: String,
    },
}

impl ConfigError {
    /// Build a [`ConfigError::Other`] from any displayable value.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}
