//! Typed failures preserve validation, ownership and conflict distinctions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::managed::RevisionBundleError;
use systemprompt_traits::{BoxedSource, RepositoryError};

#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("Invalid managed resource: {0}")]
    Invalid(String),
    #[error("Invalid managed resource: {context}: {source}")]
    InvalidInput {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
    #[error("Managed resource is unavailable in this scope")]
    Unavailable,
    #[error("Managed resource conflict: {0}")]
    Conflict(String),
    #[error("Managed resource integrity check failed")]
    Integrity,
    #[error("Managed resource storage failed: {0}")]
    Repository(#[from] RepositoryError),
    #[error("Managed resource operation failed: {context}: {source}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
    #[error("Managed authoring I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Managed resource serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<sqlx::Error> for ManagedError {
    fn from(error: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(error))
    }
}

impl From<RevisionBundleError> for ManagedError {
    fn from(error: RevisionBundleError) -> Self {
        match error {
            RevisionBundleError::Invalid(message) => Self::Invalid(message),
            RevisionBundleError::Integrity => Self::Integrity,
            RevisionBundleError::MissingRevision(_) => Self::Unavailable,
            RevisionBundleError::Json(error) => Self::Json(error),
        }
    }
}

impl From<IdValidationError> for ManagedError {
    fn from(error: IdValidationError) -> Self {
        invalid_input("identifier", error)
    }
}

pub type Result<T> = std::result::Result<T, ManagedError>;

pub(super) fn invalid(message: &str) -> ManagedError {
    ManagedError::Invalid(message.to_owned())
}

pub(crate) fn invalid_input(context: &'static str, source: impl Into<BoxedSource>) -> ManagedError {
    ManagedError::InvalidInput {
        context,
        source: source.into(),
    }
}


// Why: `Integrity` carries no payload by contract, so the cause is retained in
// the log at the one place it is still known.
pub(crate) fn integrity(error: impl std::fmt::Display) -> ManagedError {
    tracing::warn!(error = %error, "Retained managed data failed an integrity check");
    ManagedError::Integrity
}
