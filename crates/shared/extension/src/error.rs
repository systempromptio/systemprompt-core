//! Typed error enums raised by extension registration and configuration.

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

    #[error("Migration failed for extension '{extension}': {message}")]
    MigrationFailed { extension: String, message: String },

    #[error(
        "Migration {version} for extension '{extension}' is not reversible (no down SQL provided)"
    )]
    MigrationNotReversible { extension: String, version: u32 },

    #[error("Configuration validation failed for extension '{extension}': {message}")]
    ConfigValidationFailed { extension: String, message: String },

    #[error("Extension '{extension}' uses reserved API path '{path}'")]
    ReservedPathCollision { extension: String, path: String },

    #[error("Extension '{extension}' has invalid base path '{path}': must start with /api/")]
    InvalidBasePath { extension: String, path: String },

    #[error("Circular dependency detected: {chain}")]
    CircularDependency { chain: String },

    #[error("Dependency cycle detected while ordering extensions: {chain}")]
    DependencyCycle { chain: String },

    #[error(
        "Extension '{extension}' migration ALTERs table '{table}' but does not create it in its \
         schemas() nor declare it in cross_extension_tables(); cross-extension table mutations \
         must be declared explicitly"
    )]
    CrossExtensionAlterUndeclared { extension: String, table: String },

    #[error(
        "Table '{table}' is created by both extension '{extension_a}' and '{extension_b}'; every \
         table must be declared by exactly one extension"
    )]
    DuplicateTableOwner {
        table: String,
        extension_a: String,
        extension_b: String,
    },

    #[error(
        "Extension '{extension}' declares cross_extension_tables() entry '{table}', which is not \
         a table created by any other loaded extension"
    )]
    CrossExtensionTableNotOwned { extension: String, table: String },

    #[error(
        "Extension '{extension}' seed '{seed}' contains forbidden statement '{statement}'; seeds \
         may only contain INSERT … ON CONFLICT, UPDATE, MERGE, or WITH … INSERT"
    )]
    InvalidSeedStatement {
        extension: String,
        seed: String,
        statement: String,
    },

    #[error(
        "Extension '{extension}' seed '{seed}' contains a bare INSERT with no ON CONFLICT clause; \
         seeds run on every boot and must be idempotent — add ON CONFLICT … DO NOTHING/UPDATE"
    )]
    SeedInsertNotIdempotent { extension: String, seed: String },

    #[error("Extension '{extension}' seed '{seed}' failed to parse or apply: {message}")]
    SeedFailed {
        extension: String,
        seed: String,
        message: String,
    },
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
