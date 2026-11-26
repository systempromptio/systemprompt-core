//! Scheduled jobs of the agent domain: the empty-context sweep, the
//! `user_context_counters` projection drainer and the
//! `agent_task_rollup_hourly` drain `backfill_rollups` runs, contributed by
//! [`AgentExtension`](crate::AgentExtension).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cleanup_empty_contexts;
pub(crate) mod rollups;
mod user_context_counters_rollup;

pub use cleanup_empty_contexts::CleanupEmptyContextsJob;
pub use user_context_counters_rollup::UserContextCountersRollupJob;
