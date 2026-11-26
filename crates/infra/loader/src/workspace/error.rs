//! Errors of the workspace layer: the CLI session store and its files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WorkspaceError {
    #[error("No CLI session file at {path}.\n\nRun: systemprompt admin session login")]
    SessionNotFound { path: String },

    #[error("Session file {path} is corrupted.\n\nRun: systemprompt admin session login")]
    SessionFileCorrupted {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "Session file version mismatch: expected {min}-{max}, got {actual}. Delete {path} and \
         retry."
    )]
    SessionVersionMismatch {
        min: u32,
        max: u32,
        actual: u32,
        path: String,
    },

    #[error(
        "Session store at {path} is stale or corrupt and cannot be parsed.\n\nRun: systemprompt \
         admin session switch <profile> to reset it"
    )]
    SessionStoreCorrupted {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
