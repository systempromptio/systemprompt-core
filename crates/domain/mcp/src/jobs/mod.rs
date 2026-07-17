//! Scheduled background jobs for the MCP domain.
//!
//! Houses the periodic session-cleanup job that expires and prunes stale
//! `mcp_sessions` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod mcp_session_cleanup;
