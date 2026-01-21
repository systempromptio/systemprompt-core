use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Extension '{extension}' requires dependency '{dependency}' which is not registered")]
    MissingDependency {
        extension: String,
        dependency: String,
    },

    #[error("Extension with ID '{0}' is already registered")]
    DuplicateExtension(String),

    #[error("Failed to initialize extension '{extension}': {message}")]
    InitializationFailed { extension: String, message: String },

    #[error("Failed to install schema for extension '{extension}': {message}")]
    SchemaInstallationFailed { extension: String, message: String },

    #[error("Configuration validation failed for extension '{extension}': {message}")]
    ConfigValidationFailed { extension: String, message: String },

    #[error("Extension '{extension}' uses reserved API path '{path}'")]
    ReservedPathCollision { extension: String, path: String },

    #[error("Extension '{extension}' has invalid base path '{path}': must start with /api/")]
    InvalidBasePath { extension: String, path: String },

    #[error("Circular dependency detected: {chain}")]
    CircularDependency { chain: String },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration key '{0}' not found")]
    NotFound(String),

    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidValue { key: String, message: String },

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}
