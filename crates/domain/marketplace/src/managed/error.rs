//! Typed failures preserve validation, ownership and conflict distinctions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::bridge::host::UnknownHostKind;
use systemprompt_models::managed::RevisionBundleError;
use systemprompt_traits::{BoxedSource, RepositoryError};

#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("Invalid managed resource: {reason}")]
    Invalid { reason: &'static str },
    #[error("Invalid managed resource: Inventory names must be UTF-8: {name}")]
    NonUtf8EntryName { name: String },
    #[error("Invalid managed resource: Skill hosts list is invalid: {source}")]
    SkillHosts {
        #[source]
        source: UnknownHostKind,
    },
    #[error(transparent)]
    InvalidBundle(RevisionBundleError),
    #[error("Invalid managed resource: {context}: {source}")]
    InvalidInput {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
    #[error("Managed resource is unavailable in this scope")]
    Unavailable,
    #[error("Managed resource conflict: {reason}")]
    Conflict { reason: &'static str },
    #[error("Managed resource conflict: Publication gate {gate} refused the publication: {reason}")]
    GateRefused { gate: &'static str, reason: String },
    #[error(
        "Managed resource conflict: Resolved revision differs from the recorded merge at {path}"
    )]
    MergeMismatch { path: String },
    #[error("Managed resource conflict: Expected publication generation {expected}, found {found}")]
    GenerationMismatch { expected: i64, found: i64 },
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

impl ManagedError {
    pub const fn is_invalid(&self) -> bool {
        matches!(
            self,
            Self::Invalid { .. }
                | Self::NonUtf8EntryName { .. }
                | Self::SkillHosts { .. }
                | Self::InvalidBundle(_)
        )
    }

    pub const fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::Conflict { .. }
                | Self::GateRefused { .. }
                | Self::MergeMismatch { .. }
                | Self::GenerationMismatch { .. }
        )
    }
}

impl From<sqlx::Error> for ManagedError {
    fn from(error: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(error))
    }
}

impl From<RevisionBundleError> for ManagedError {
    fn from(error: RevisionBundleError) -> Self {
        match error {
            invalid @ RevisionBundleError::Invalid { .. } => Self::InvalidBundle(invalid),
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

pub(super) const fn invalid(reason: &'static str) -> ManagedError {
    ManagedError::Invalid { reason }
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
