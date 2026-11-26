//! Scheduled background jobs for the MCP domain.
//!
//! Houses the periodic session-cleanup job that expires and prunes stale
//! `mcp_sessions` rows, the `tool_row_facts` projection drainer, the
//! `mcp_tool_executions` retention purge `database_cleanup` dispatches to,
//! and the `tool_usage_rollup_hourly` drain `backfill_rollups` runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod mcp_session_cleanup;
mod retention;
mod rollups;
mod tool_row_facts_rollup;

pub use mcp_session_cleanup::McpSessionCleanupJob;
pub use tool_row_facts_rollup::ToolRowFactsRollupJob;

pub(crate) fn contribute(c: &mut systemprompt_extension::Contributions<'_>) {
    c.job(McpSessionCleanupJob).job(ToolRowFactsRollupJob);
    retention::contribute(c);
    rollups::contribute(c);
}
