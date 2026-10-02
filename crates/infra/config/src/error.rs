//! Public error type for `systemprompt-config`.
//!
//! All public APIs of this crate return [`ConfigError`] (or
//! [`ConfigResult<T>`]) instead of `anyhow::Error`. The enum is
//! `#[non_exhaustive]` so additional variants can be added in patch
//! releases without breaking downstream code that performs exhaustive
//! matching only on the documented variants.
//!
//! Upstream errors are composed via `#[from]` so callers can use `?`
//! transparently from `std::io`, `serde_json` and `serde_yaml`
//! operations performed inside the bootstrap pipeline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_models::errors::{GlobalConfigError, SecretsError};
use systemprompt_models::profile::ProfileError;

use crate::bootstrap::{ProfileBootstrapError, SecretsBootstrapError};
use crate::services::ConfigValidationError;

pub type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("Config already initialized")]
    AlreadyInitialized,

    #[error(
        "cannot resolve a stable instance id for a cloud profile: set server.instance_id or \
         export HOSTNAME so this replica keeps one identity across restarts"
    )]
    InstanceIdUnresolved,

    #[error(transparent)]
    Profile(#[from] ProfileBootstrapError),

    #[error(transparent)]
    Secrets(#[from] SecretsBootstrapError),

    #[error(transparent)]
    ProfileParse(#[from] ProfileError),

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

    #[error("Invalid database URL: {0}")]
    InvalidDatabaseUrl(#[source] GlobalConfigError),

    #[error(
        "Profile is missing required `system_admin.username` and `SYSTEMPROMPT_SYSTEM_ADMIN` is \
         not set. The platform refuses to start without an explicit system-admin identity."
    )]
    MissingSystemAdmin,

    #[error("No provider named {name}")]
    ProviderNotFound { name: ProviderId },

    #[error("No model with id {id} under provider {provider}")]
    ModelNotFound { id: ModelId, provider: ProviderId },

    #[error("Access token expiry must be positive")]
    NonPositiveAccessTokenExpiry,

    #[error("Refresh token expiry must be positive")]
    NonPositiveRefreshTokenExpiry,

    #[error("No trusted issuer found with issuer {issuer}")]
    TrustedIssuerNotFound { issuer: String },

    #[error("Profile path has no parent directory: {path}")]
    ProfilePathWithoutParent { path: PathBuf },

    #[error("Secrets file root is not a JSON object: {path}")]
    SecretsFileNotObject { path: PathBuf },
}
