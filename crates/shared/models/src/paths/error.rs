//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
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

    #[error("Binary not found: {name} (searched: {searched:?})")]
    BinaryNotFound {
        name: String,
        searched: Vec<PathBuf>,
    },

    #[error("Failed to resolve the running executable: {source}")]
    CurrentExeUnavailable {
        #[source]
        source: std::io::Error,
    },
}
