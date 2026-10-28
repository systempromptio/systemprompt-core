//! The revision engine's pure parts: deciding how a commit lands
//! ([`CommitPlan`]) and three-way merging text ([`three_way`]).
//!
//! Neither touches the database or storage, so the commit algorithm can be
//! tested without either; `DocumentService::commit` executes the plan.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod commit;
mod merge;

pub use commit::{CommitBody, CommitPlan};
pub use merge::{MergeOutcome, three_way, three_way_bytes};
