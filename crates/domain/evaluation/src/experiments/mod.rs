//! Versioned experiment inputs, evidence-based scoring, and budget admission.
//!
//! Build a frozen matrix with [`ExperimentSpec::builder`], persist its typed
//! resource revisions with [`RevisionRepository`], and queue it with
//! [`ExperimentRepository`]. Budget admission returns [`ReservationAdmission`]:
//! only `Admitted` permits a new dispatch; `AlreadyReserved` is reconciliation
//! state and must never dispatch another provider call. A timeout does not
//! release a reservation. Monetary values are integer microdollars.
//!
//! Repositories are constructed once with the application write pool and
//! injected into the owning application service. Domain failures use
//! [`crate::EvaluationError`]; callers retain the error variant at their HTTP
//! or CLI boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builder;
pub mod execution;
mod model;
pub use builder::ExperimentSpecBuilder;
pub mod records;
pub mod resources;
pub mod scoring;

pub use crate::repository::experiments::{
    BudgetRepository, ExperimentRepository, ReservationAdmission, RevisionRepository,
};
pub use model::{ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec};

use crate::{EvaluationError, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn content_digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_jcs::to_vec(value)?)))
}

pub(crate) fn invalid(message: &str) -> EvaluationError {
    EvaluationError::InvalidSpec(message.to_owned())
}

pub(crate) fn conflict(message: &str) -> EvaluationError {
    EvaluationError::ExperimentConflict(message.to_owned())
}

pub(crate) fn missing(message: &str) -> EvaluationError {
    EvaluationError::ResourceNotFound(message.to_owned())
}
