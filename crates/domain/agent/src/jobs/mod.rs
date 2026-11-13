//! Scheduled jobs of the agent domain: the empty-context sweep and the
//! `user_context_counters` projection drainer, submitted with `submit_job!`
//! and returned from the extension's `jobs()`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cleanup_empty_contexts;
mod user_context_counters_rollup;

pub use cleanup_empty_contexts::CleanupEmptyContextsJob;
pub use user_context_counters_rollup::UserContextCountersRollupJob;
