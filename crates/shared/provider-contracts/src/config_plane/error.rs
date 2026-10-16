//! [`ConfigPlaneError`]: failures of a plane's read, apply or write guard.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use crate::dependencies::MissingDependency;

pub type ConfigPlaneResult<T> = Result<T, ConfigPlaneError>;

#[derive(Debug, Error)]
pub enum ConfigPlaneError {
    #[error("config plane '{plane}': declaration {path} could not be read: {source}")]
    Unreadable {
        plane: String,
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("config plane '{plane}': declaration {path} is invalid: {message}")]
    Invalid {
        plane: String,
        path: String,
        message: String,
    },

    #[error("config plane '{plane}': {source}")]
    Backend {
        plane: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error(transparent)]
    MissingDependency(#[from] MissingDependency),

    #[error(
        "config plane '{plane}' is file-managed: {path} is the source of truth and the \
         reconciler overwrites database edits; change the file instead"
    )]
    FileManaged { plane: String, path: String },

    #[error("no config plane named '{0}' is registered")]
    UnknownPlane(String),

    #[error("config plane '{0}' is registered by more than one extension")]
    DuplicatePlane(String),

    #[error("config plane '{plane}': apply did not converge, {remaining} drift rows remain")]
    NotConverged { plane: String, remaining: usize },
}
