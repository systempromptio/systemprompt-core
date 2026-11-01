//! Error types for the brain seams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{BoxedSource, RepositoryError};

/// Why emitting a brain event failed.
#[derive(Debug, thiserror::Error)]
pub enum BrainSinkError {
    #[error("brain event could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[cfg(feature = "sqlx")]
    #[error("brain outbox write failed")]
    Database(#[from] sqlx::Error),
    #[error("brain outbox write failed")]
    Repository(#[from] RepositoryError),
    #[error("the brain is not enabled on this server")]
    Unavailable,
}

/// Why a model call through the gateway failed.
#[derive(Debug, thiserror::Error)]
pub enum BrainModelError {
    #[error("the brain model client is unavailable")]
    Unavailable,
    #[error("brain model call timed out")]
    Timeout,
    #[error("brain model call was refused: {reason}")]
    Refused { reason: String },
    #[error("brain model output could not be decoded")]
    Decode(#[from] serde_json::Error),
    #[error("brain model call failed")]
    Other(#[source] BoxedSource),
}

/// Why an extractor could not produce candidates.
#[derive(Debug, thiserror::Error)]
pub enum BrainExtractError {
    #[error("extraction model call failed")]
    Model(#[from] BrainModelError),
    #[error("extractor output could not be decoded")]
    Decode(#[from] serde_json::Error),
    #[error("extractor {extractor} failed")]
    Other {
        extractor: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why a validator could not reach a verdict.
#[derive(Debug, thiserror::Error)]
pub enum BrainValidateError {
    #[cfg(feature = "sqlx")]
    #[error("brain validator query failed")]
    Database(#[from] sqlx::Error),
    #[error("brain validator query failed")]
    Repository(#[from] RepositoryError),
    #[error("brain validator {validator} failed")]
    Other {
        validator: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why a source's text could not be read.
#[derive(Debug, thiserror::Error)]
pub enum SourceContentError {
    #[cfg(feature = "sqlx")]
    #[error("source content query failed")]
    Database(#[from] sqlx::Error),
    #[error("source content query failed")]
    Repository(#[from] RepositoryError),
    #[error("source content {source_kind} failed")]
    Other {
        source_kind: String,
        #[source]
        source: BoxedSource,
    },
}

/// Why a recall tier, retriever or receipt failed.
#[derive(Debug, thiserror::Error)]
pub enum RecallError {
    #[cfg(feature = "sqlx")]
    #[error("brain recall query failed")]
    Database(#[from] sqlx::Error),
    #[error("brain recall query failed")]
    Repository(#[from] RepositoryError),
    #[error("brain recall {component} failed")]
    Other {
        component: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why a source's readability could not be decided.
#[derive(Debug, thiserror::Error)]
pub enum SourceAccessError {
    #[cfg(feature = "sqlx")]
    #[error("source access query failed")]
    Database(#[from] sqlx::Error),
    #[error("source access query failed")]
    Repository(#[from] RepositoryError),
    #[error("source access {kind} failed")]
    Other {
        kind: String,
        #[source]
        source: BoxedSource,
    },
}

/// Why an outcome signal could not observe a recall.
#[derive(Debug, thiserror::Error)]
pub enum OutcomeSignalError {
    #[cfg(feature = "sqlx")]
    #[error("outcome signal query failed")]
    Database(#[from] sqlx::Error),
    #[error("outcome signal query failed")]
    Repository(#[from] RepositoryError),
    #[error("outcome signal {signal} failed")]
    Other {
        signal: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why an analysis could not draft proposals.
#[derive(Debug, thiserror::Error)]
pub enum BrainAnalysisError {
    #[cfg(feature = "sqlx")]
    #[error("brain analysis query failed")]
    Database(#[from] sqlx::Error),
    #[error("brain analysis query failed")]
    Repository(#[from] RepositoryError),
    #[error("brain analysis {analysis} failed")]
    Other {
        analysis: &'static str,
        #[source]
        source: BoxedSource,
    },
}
