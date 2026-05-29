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

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("Config already initialized")]
    AlreadyInitialized,

    #[error(transparent)]
    Profile(#[from] ProfileBootstrapError),

    #[error(transparent)]
    Secrets(#[from] SecretsBootstrapError),

    #[error(transparent)]
    ProfileParse(#[from] ProfileError),

    #[error(transparent)]
    Gateway(#[from] GatewayProfileError),

    #[error(transparent)]
    SchemaValidation(#[from] ConfigValidationError),

    #[error(transparent)]
    SecretsParse(#[from] SecretsError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error("Missing required path: paths.{field}")]
    MissingProfilePath { field: String },

    #[error("Failed to canonicalize {name} path: {source}")]
    CanonicalizePath {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Profile path '{field}' cannot be read: {path}")]
    ReadProfilePath {
        field: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Profile path '{field}' has invalid YAML: {path}")]
    InvalidProfileYaml {
        field: String,
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Profile path validation failed: {message}")]
    ProfilePathReport { message: String },

    #[error("Unsupported database type '{db_type}'. Only 'postgres' is supported.")]
    UnsupportedDatabaseType { db_type: String },

    #[error("Invalid database URL: {message}")]
    InvalidDatabaseUrl { message: String },

    #[error("Failed to resolve variables after {passes} passes: {unresolved}")]
    UnresolvedVariables { passes: usize, unresolved: String },

    #[error("{count} validation error(s)")]
    ValidationErrors { count: usize },

    #[error("Required config file missing: {path}")]
    EnvironmentConfigMissing { path: PathBuf },

    #[error(
        "Profile is missing required `system_admin.username` and `SYSTEMPROMPT_SYSTEM_ADMIN` is \
         not set. The platform refuses to start without an explicit system-admin identity."
    )]
    MissingSystemAdmin,

    #[error("{message}")]
    Other { message: String },
}

impl ConfigError {
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}
