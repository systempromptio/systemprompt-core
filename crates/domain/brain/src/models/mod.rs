//! Crate-local row shapes: what the repositories insert and what they read
//! back before it becomes a `systemprompt_models::brain` value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod change;
mod fact;
mod ledger;
mod plane;
mod read;
mod row;

pub use change::{FactChange, NewRevision};
pub use fact::{Inserted, NewFact, NewFactBuilder};
pub use ledger::{DisputeStatus, EventOutcome, EventResult, NewDispute, NewExtraction, NewFailure};
pub use plane::{PlaneRow, PlaneSource};
pub use read::{FactFilter, NewOutcome, NewProposal, NewRecall, NewRecallBuilder};
pub(crate) use row::{FactRow, FieldMap, RevisionRow, decode_status};
