//! Scheduled background jobs for the MCP domain.
//!
//! Houses the periodic session-cleanup job that expires and prunes stale
//! `mcp_sessions` rows, the `tool_row_facts` projection drainer, and the
//! `mcp_tool_executions` retention purge `database_cleanup` dispatches to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod mcp_session_cleanup;
mod retention;
mod tool_row_facts_rollup;

pub use tool_row_facts_rollup::ToolRowFactsRollupJob;
