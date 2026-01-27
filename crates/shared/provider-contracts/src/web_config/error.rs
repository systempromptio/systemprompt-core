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
