//! Durable three-way reconciliation for managed and incoming revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod model;
mod repository;

pub use model::{
    ConflictDecision, ConflictResolution, ReconciliationConflict, ReconciliationRecord,
    ReconciliationRequest, ReconciliationStatus,
};
