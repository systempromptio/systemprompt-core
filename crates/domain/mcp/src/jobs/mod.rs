//! Scheduled background jobs for the MCP domain.
//!
//! Houses the periodic session-cleanup job that expires and prunes stale
//! `mcp_sessions` rows.

mod mcp_session_cleanup;
