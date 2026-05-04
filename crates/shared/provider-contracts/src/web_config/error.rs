//! Typed error returned by [`crate::web_config::WebConfig`] loading.

use std::path::PathBuf;

use thiserror::Error;

/// Failure modes surfaced while loading or validating a `WebConfig`.
#[derive(Debug, Error)]
pub enum WebConfigError {
    /// I/O failure while reading the YAML file.
    #[error("Failed to read web config at '{path}': {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// YAML parse failure.
    #[error("Failed to parse web config: {0}")]
    Parse(#[from] serde_yaml::Error),

    /// A required YAML field was missing.
    #[error("Missing required field: {field}")]
    MissingField {
        /// Name of the missing field.
        field: String,
    },

    /// A field was present but had an invalid value.
    #[error("Invalid value for {field}: {message}")]
    InvalidValue {
        /// Name of the offending field.
        field: String,
        /// Human-readable detail.
        message: String,
    },

    /// A configured directory path did not resolve.
    #[error("{field} directory not found: {path}")]
    PathNotFound {
        /// Name of the offending field.
        field: String,
        /// Path that failed to resolve.
        path: PathBuf,
    },
}
