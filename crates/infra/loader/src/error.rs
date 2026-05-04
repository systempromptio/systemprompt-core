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

/// Failures raised while reading, parsing, and merging a services config.
#[derive(Debug, Error)]
pub enum ConfigLoadError {
    /// The profile bootstrap has not yet been initialised, so the active
    /// services-config path is unknown.
    #[error("profile bootstrap unavailable: {0}")]
    ProfileBootstrap(String),

    /// A file operation failed (read, canonicalise, etc.).
    #[error("io error at {path}: {source}")]
    Io {
        /// File the operation was attempted on.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// YAML deserialisation failed.
    #[error("yaml parse failure at {path}: {source}")]
    Yaml {
        /// File whose content failed to parse.
        path: PathBuf,
        /// Underlying serde failure.
        #[source]
        source: serde_yaml::Error,
    },

    /// An `includes:` entry pointed at a non-existent file.
    #[error(
        "include file not found: {include}\nReferenced in: {referrer}\nEither create the file or \
         remove it from the includes list."
    )]
    IncludeNotFound {
        /// Path that the include statement resolved to.
        include: PathBuf,
        /// File containing the offending include directive.
        referrer: PathBuf,
    },

    /// The include graph contained a cycle.
    #[error("include cycle detected: {chain}")]
    IncludeCycle {
        /// Arrow-joined chain of include sites that closes the cycle.
        chain: String,
    },

    /// Two include files contributed the same agent name.
    #[error("duplicate agent definition: {0}")]
    DuplicateAgent(String),

    /// Two include files contributed the same MCP server name.
    #[error("duplicate MCP server definition: {0}")]
    DuplicateMcpServer(String),

    /// Two include files contributed the same plugin name.
    #[error("duplicate plugin definition: {0}")]
    DuplicatePlugin(String),

    /// Two include files contributed the same skill id.
    #[error("duplicate skill definition: {0}")]
    DuplicateSkill(String),

    /// Two include files contributed the same content source name.
    #[error("duplicate content source definition: {0}")]
    DuplicateContentSource(String),

    /// The merged configuration failed semantic validation.
    #[error("services config validation failed: {0}")]
    Validation(String),
}

/// Failures raised while writing a services config (agent files, removing
/// includes, etc.).
#[derive(Debug, Error)]
pub enum ConfigWriteError {
    /// A file operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// File the operation was attempted on.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// YAML serialisation failed.
    #[error("yaml serialisation failed: {0}")]
    YamlEncode(#[from] serde_yaml::Error),

    /// The target agent file already exists.
    #[error("Agent file already exists: {0}. Use 'agents edit' to modify.")]
    AgentFileExists(PathBuf),

    /// The named agent could not be located in any configuration file.
    #[error("Agent '{0}' not found in any configuration file")]
    AgentNotFound(String),
}

/// Failures raised by the extension registry / loader.
#[derive(Debug, Error)]
pub enum ExtensionLoadError {
    /// A binary listed in the registry could not be located on disk.
    #[error("Binary '{name}' not found at {path}")]
    BinaryNotFound {
        /// Binary name that was requested.
        name: String,
        /// Filesystem path that was probed.
        path: PathBuf,
    },

    /// No `manifest.yaml` was found for the requested extension.
    #[error("No manifest.yaml found for extension '{0}' in extensions/")]
    ManifestMissing(String),
}

/// Convenience [`Result`] alias parameterised on [`ConfigLoadError`].
pub type ConfigLoadResult<T> = Result<T, ConfigLoadError>;

/// Convenience [`Result`] alias parameterised on [`ConfigWriteError`].
pub type ConfigWriteResult<T> = Result<T, ConfigWriteError>;

/// Convenience [`Result`] alias parameterised on [`ExtensionLoadError`].
pub type ExtensionLoadResult<T> = Result<T, ExtensionLoadError>;

/// Failures raised by [`crate::ProfileLoader`].
///
/// Composes `systemprompt_models::profile::ProfileError` for parse and
/// validation failures with crate-local I/O errors so callers see a
/// single Result type.
#[derive(Debug, Error)]
pub enum ProfileLoadError {
    /// A file operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// File the operation was attempted on.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// Upstream profile parse, validation, or serialization failure.
    #[error(transparent)]
    Profile(#[from] systemprompt_models::profile::ProfileError),
}

/// Convenience [`Result`] alias parameterised on [`ProfileLoadError`].
pub type ProfileLoadResult<T> = Result<T, ProfileLoadError>;
