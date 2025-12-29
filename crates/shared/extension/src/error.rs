//! Extension loader error types.
//!
//! This module contains [`LoaderError`] for extension loading and registration
//! failures. For the error trait that extensions implement for HTTP/MCP
//! responses, see [`systemprompt_traits::ExtensionError`].

use thiserror::Error;

/// Errors that occur during extension loading, registration, and
/// initialization.
///
/// These errors represent failures in the extension loader subsystem,
/// including:
/// - Dependency resolution (missing or circular dependencies)
/// - Registration conflicts (duplicate IDs, path collisions)
/// - Schema installation failures
/// - Configuration validation failures
#[derive(Debug, Error)]
pub enum LoaderError {
    /// A required dependency is missing.
    #[error("Extension '{extension}' requires dependency '{dependency}' which is not registered")]
    MissingDependency {
        /// The extension that has the unmet dependency.
        extension: String,
        /// The missing dependency ID.
        dependency: String,
    },

    /// An extension with the same ID is already registered.
    #[error("Extension with ID '{0}' is already registered")]
    DuplicateExtension(String),

    /// Extension initialization failed.
    #[error("Failed to initialize extension '{extension}': {message}")]
    InitializationFailed {
        /// The extension that failed to initialize.
        extension: String,
        /// The error message.
        message: String,
    },

    /// Schema installation failed.
    #[error("Failed to install schema for extension '{extension}': {message}")]
    SchemaInstallationFailed {
        /// The extension whose schema failed.
        extension: String,
        /// The error message.
        message: String,
    },

    /// Configuration validation failed.
    #[error("Configuration validation failed for extension '{extension}': {message}")]
    ConfigValidationFailed {
        /// The extension with invalid config.
        extension: String,
        /// The validation error message.
        message: String,
    },

    /// API path collision with reserved paths.
    #[error("Extension '{extension}' uses reserved API path '{path}'")]
    ReservedPathCollision {
        /// The extension that caused the collision.
        extension: String,
        /// The reserved path.
        path: String,
    },

    /// Invalid API base path.
    #[error("Extension '{extension}' has invalid base path '{path}': must start with /api/")]
    InvalidBasePath {
        /// The extension with invalid path.
        extension: String,
        /// The invalid path.
        path: String,
    },

    /// Circular dependency detected.
    #[error("Circular dependency detected: {chain}")]
    CircularDependency {
        /// The dependency chain that forms the cycle.
        chain: String,
    },
}

/// Configuration error for extensions.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Configuration key not found.
    #[error("Configuration key '{0}' not found")]
    NotFound(String),

    /// Configuration value is invalid.
    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue {
        /// The configuration key.
        key: String,
        /// The error message.
        message: String,
    },

    /// Configuration parsing failed.
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Schema validation failed.
    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}
