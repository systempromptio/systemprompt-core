//! Typed error enums raised by extension registration and configuration.

use thiserror::Error;

/// Failure raised by [`crate::ExtensionRegistry`], [`crate::ExtensionBuilder`],
/// and [`crate::TypedExtensionRegistry`] when an extension cannot be loaded
/// or installed.
#[derive(Debug, Error)]
pub enum LoaderError {
    /// An extension declared a dependency that is not registered.
    #[error("Extension '{extension}' requires dependency '{dependency}' which is not registered")]
    MissingDependency {
        /// ID of the extension whose dependency is missing.
        extension: String,
        /// ID of the missing dependency.
        dependency: String,
    },

    /// Two extensions registered the same ID.
    #[error("Extension with ID '{0}' is already registered")]
    DuplicateExtension(String),

    /// An extension's `init` step failed.
    #[error("Failed to initialize extension '{extension}': {message}")]
    InitializationFailed {
        /// ID of the failing extension.
        extension: String,
        /// Underlying error message.
        message: String,
    },

    /// Schema installation for an extension failed.
    #[error("Failed to install schema for extension '{extension}': {message}")]
    SchemaInstallationFailed {
        /// ID of the extension whose schema could not be installed.
        extension: String,
        /// Underlying error message.
        message: String,
    },

    /// A migration belonging to an extension failed.
    #[error("Migration failed for extension '{extension}': {message}")]
    MigrationFailed {
        /// ID of the extension whose migration failed.
        extension: String,
        /// Underlying error message.
        message: String,
    },

    /// A configuration block failed validation against the extension's
    /// schema.
    #[error("Configuration validation failed for extension '{extension}': {message}")]
    ConfigValidationFailed {
        /// ID of the extension whose configuration is invalid.
        extension: String,
        /// Underlying error message.
        message: String,
    },

    /// An extension tried to mount a router under a reserved API path.
    #[error("Extension '{extension}' uses reserved API path '{path}'")]
    ReservedPathCollision {
        /// ID of the offending extension.
        extension: String,
        /// Reserved path that was requested.
        path: String,
    },

    /// An extension's base path does not start with `/api/`.
    #[error("Extension '{extension}' has invalid base path '{path}': must start with /api/")]
    InvalidBasePath {
        /// ID of the offending extension.
        extension: String,
        /// Invalid base path.
        path: String,
    },

    /// A circular dependency was detected during registration.
    #[error("Circular dependency detected: {chain}")]
    CircularDependency {
        /// `->`-separated chain of extension IDs forming the cycle.
        chain: String,
    },
}

/// Failure raised when an extension's configuration cannot be parsed or
/// validated.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required configuration key was not present.
    #[error("Configuration key '{0}' not found")]
    NotFound(String),

    /// A configuration key contained an invalid value.
    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue {
        /// Configuration key whose value is invalid.
        key: String,
        /// Description of why the value was rejected.
        message: String,
    },

    /// Configuration parsing failed (e.g. malformed JSON).
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Configuration failed JSON-schema validation.
    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}
