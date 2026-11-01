//! [`BrainError`]: every failure the brain store, pipeline and services
//! report.
//!
//! [`BrainError::is_transient`] separates a failure worth retrying
//! (storage, the outbox sink) from one that fails the same way every time
//! (an unknown kind, a tombstoned identity, a refused action).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::{
    BrainExtractError, BrainModelError, BrainSinkError, BrainValidateError, SourceContentError,
};
use systemprompt_extension::notifications::NotificationSinkError;
use systemprompt_identifiers::{BrainFactId, BrainProposalId, FactKindId};
use systemprompt_models::brain::{StatementError, SubjectKeyError};
use systemprompt_models::notifications::NotificationKindError;
use systemprompt_traits::RepositoryError;

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
    #[error("proposal {0} not found")]
    ProposalNotFound(BrainProposalId),
    #[error("brain event kind {0} is not one the pipeline reads")]
    UnknownEvent(String),
    #[error("brain extraction failed")]
    Extract(#[from] BrainExtractError),
    #[error("brain validator failed")]
    Validate(#[from] BrainValidateError),
    #[error("brain source content could not be read")]
    Source(#[from] SourceContentError),
    #[error("brain source content timed out")]
    SourceTimeout,
    #[error("brain notification could not be emitted")]
    Notify(#[from] NotificationSinkError),
    #[error("brain notification kind is invalid")]
    NotifyKind(#[from] NotificationKindError),
    #[error("the daily extraction token budget is spent")]
    BudgetExhausted,
}

impl BrainError {
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        match self {
            Self::Database(_)
            | Self::Repository(_)
            | Self::Sink(_)
            | Self::Validate(_)
            | Self::Source(_)
            | Self::SourceTimeout
            | Self::Notify(_)
            | Self::BudgetExhausted => true,
            Self::Extract(BrainExtractError::Model(error)) => matches!(
                error,
                BrainModelError::Unavailable | BrainModelError::Timeout | BrainModelError::Other(_)
            ),
            _ => false,
        }
    }

    #[must_use]
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::Extract(_) => "extract",
            Self::Validate(_) => "validate",
            Self::Source(_) | Self::SourceTimeout => "source",
            Self::Notify(_) => "notify",
            Self::BudgetExhausted => "budget",
            _ => "process",
        }
    }
}
