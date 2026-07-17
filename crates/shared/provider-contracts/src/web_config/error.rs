//! Typed error returned by [`crate::web_config::WebConfig`] loading.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebConfigError {
    #[error("Failed to read web config at '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse web config: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    #[error("{field} directory not found: {path}")]
    PathNotFound { field: String, path: PathBuf },
}
