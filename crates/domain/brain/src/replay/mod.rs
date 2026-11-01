//! Replay: re-running the capture pipeline over a window of past events
//! under a chosen pipeline version, as an explicit dry run with a diff.
//!
//! [`ReplayService::dry_run`] reads the brain consumer's events through
//! `OutboxConsumer::replay_window` (the brain never reads the outbox
//! table), processes each on a transaction that is rolled back, and
//! reports what the facts would have become ([`ReplayReport`]). The report
//! is attached to a proposal; nothing a replay computes is kept otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod budget;
pub mod diff;
mod run;
mod store;

pub use budget::CountingModel;
pub use diff::{ChangeCounts, FactState, ReplayChange, ReplayReport, classify, tally};
pub use run::{ProposalReplay, ReplayService};
pub use store::ReplayRepository;
