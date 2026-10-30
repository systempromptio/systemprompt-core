//! [`BrainError`]: every failure the brain store, pipeline and services
//! report.
//!
//! [`BrainError::is_transient`] separates a failure worth retrying
//! (storage, the outbox sink) from one that fails the same way every time
//! (an unknown kind, a tombstoned identity, a refused action).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::BrainSinkError;
use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::brain::{StatementError, SubjectKeyError};
use systemprompt_traits::{BoxedSource, RepositoryError};

pub type BrainResult<T> = Result<T, BrainError>;

/// Why a brain operation failed.
#[derive(Debug, thiserror::Error)]
pub enum BrainError {
    #[error("brain database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("brain storage failed")]
    Repository(#[from] RepositoryError),
    #[error("brain value could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[error("brain event could not be emitted")]
    Sink(#[from] BrainSinkError),
    #[error("fact statement could not be rendered")]
    Statement(#[from] StatementError),
    #[error("invalid fact subject")]
    Subject(#[from] SubjectKeyError),
    #[error("fact kind {0} is not declared")]
    UnknownKind(FactKindId),
    #[error("the fact identity was erased and may not be recorded again")]
    Tombstoned,
    #[error("brain action refused: {reason}")]
    Forbidden { reason: String },
    #[error("fact {0} not found")]
    NotFound(BrainFactId),
    #[error("brain notification could not be emitted")]
    Notify(#[source] BoxedSource),
}

impl BrainError {
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Database(_) | Self::Repository(_) | Self::Sink(_) | Self::Notify(_)
        )
    }
}
