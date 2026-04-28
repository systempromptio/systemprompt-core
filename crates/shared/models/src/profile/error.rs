use std::path::PathBuf;
use thiserror::Error;

use super::GatewayProfileError;

pub type ProfileResult<T> = Result<T, ProfileError>;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("Failed to parse profile {path}: {source}")]
    ParseYaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Failed to serialize profile: {0}")]
    SerializeYaml(#[source] serde_yaml::Error),

    #[error("Invalid profile path: {path}")]
    InvalidProfilePath { path: PathBuf },

    #[error(transparent)]
    Gateway(#[from] GatewayProfileError),

    #[error("Profile '{name}' validation failed:\n  - {}", errors.join("\n  - "))]
    Validation { name: String, errors: Vec<String> },

    #[error("Missing required environment variable: {name}")]
    MissingEnvVar { name: &'static str },

    #[error("Invalid environment variable {name}: {message}")]
    InvalidEnvVar { name: &'static str, message: String },
}
