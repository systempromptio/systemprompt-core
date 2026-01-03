use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("AppPaths not initialized - call AppPaths::init() first")]
    NotInitialized,

    #[error("AppPaths already initialized")]
    AlreadyInitialized,

    #[error("Required path not configured: {field}")]
    NotConfigured { field: &'static str },

    #[error("Path does not exist: {}", path.display())]
    NotFound { path: PathBuf, field: &'static str },

    #[error("Failed to canonicalize path {}: {source}", path.display())]
    CanonicalizeFailed {
        path: PathBuf,
        field: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("Binary not found: {name}")]
    BinaryNotFound {
        name: String,
        searched: Vec<PathBuf>,
    },
}
