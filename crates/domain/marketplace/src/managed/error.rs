//! Typed failures preserve validation, ownership and conflict distinctions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::managed::RevisionBundleError;

#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("Invalid managed resource: {0}")]
    Invalid(String),
    #[error("Managed resource is unavailable in this scope")]
    Unavailable,
    #[error("Managed resource conflict: {0}")]
    Conflict(String),
    #[error("Managed resource integrity check failed")]
    Integrity,
    #[error("Managed resource storage failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Managed repository pool unavailable: {0}")]
    Pool(#[from] systemprompt_database::RepositoryError),
    #[error("Managed authoring I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Managed resource serialization failed: {0}")]
    Json(#[from] serde_json::Error),
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

impl From<systemprompt_identifiers::error::IdValidationError> for ManagedError {
    fn from(error: systemprompt_identifiers::error::IdValidationError) -> Self {
        Self::Invalid(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ManagedError>;

pub(super) fn invalid(message: &str) -> ManagedError {
    ManagedError::Invalid(message.to_owned())
}

// Why: `Integrity` carries no payload by contract, so the cause is retained in
// the log at the one place it is still known.
pub(crate) fn integrity(error: impl std::fmt::Display) -> ManagedError {
    tracing::warn!(error = %error, "Retained managed data failed an integrity check");
    ManagedError::Integrity
}
