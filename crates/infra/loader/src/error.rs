//! Error types raised by the loader infrastructure.
//!
//! Three thiserror-derived enums cover the public surface:
//!
//! - [`ConfigLoadError`] — services-config reading, YAML parsing, include
//!   resolution, and validation failures.
//! - [`ConfigWriteError`] — file creation, agent CRUD, and config-file editing
//!   failures.
//! - [`ExtensionLoadError`] — manifest discovery and registry lookups.
//!
//! All three implement `std::error::Error` and compose with upstream
//! errors (`std::io::Error`, `serde_yaml::Error`,
//! `systemprompt_config::ProfileBootstrapError`,
//! `systemprompt_models::ServicesValidationError`,
//! `systemprompt_models::ProfileValidationError`) via `#[from]`.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("profile bootstrap unavailable: {0}")]
    ProfileBootstrap(#[from] systemprompt_config::ProfileBootstrapError),

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("yaml parse failure at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error(
        "include file not found: {include}\nReferenced in: {referrer}\nEither create the file or \
         remove it from the includes list."
    )]
    IncludeNotFound { include: PathBuf, referrer: PathBuf },

    #[error("include cycle detected: {chain}")]
    IncludeCycle { chain: String },

    #[error("duplicate agent definition: {0}")]
    DuplicateAgent(String),

    #[error("duplicate MCP server definition: {0}")]
    DuplicateMcpServer(String),

    #[error("duplicate plugin definition: {0}")]
    DuplicatePlugin(String),

    #[error("duplicate marketplace definition: {0}")]
    DuplicateMarketplace(String),

    #[error("duplicate skill definition: {0}")]
    DuplicateSkill(String),

    #[error("duplicate content source definition: {0}")]
    DuplicateContentSource(String),

    #[error("duplicate external agent definition: {0}")]
    DuplicateExternalAgent(String),

    #[error("services config validation failed: {0}")]
    Validation(String),

    #[error(
        "include {path} sets `settings:` — settings are only valid in the root config file. Move \
         the values to the root or remove them from the include."
    )]
    IncludeMustNotSetGlobalSettings { path: PathBuf },
}

#[derive(Debug, Error)]
pub enum ConfigWriteError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("yaml serialisation failed: {0}")]
    YamlEncode(#[from] serde_yaml::Error),

    #[error("Agent file already exists: {0}. Use 'agents edit' to modify.")]
    AgentFileExists(PathBuf),

    #[error("Agent '{0}' not found in any configuration file")]
    AgentNotFound(String),
}

#[derive(Debug, Error)]
pub enum ExtensionLoadError {
    #[error("Binary '{name}' not found at {path}")]
    BinaryNotFound { name: String, path: PathBuf },

    #[error("No manifest.yaml found for extension '{0}' in extensions/")]
    ManifestMissing(String),
}

pub type ConfigLoadResult<T> = Result<T, ConfigLoadError>;

pub type ConfigWriteResult<T> = Result<T, ConfigWriteError>;

pub type ExtensionLoadResult<T> = Result<T, ExtensionLoadError>;

#[derive(Debug, Error)]
pub enum ProfileLoadError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Profile(#[from] systemprompt_models::profile::ProfileError),
}

pub type ProfileLoadResult<T> = Result<T, ProfileLoadError>;
