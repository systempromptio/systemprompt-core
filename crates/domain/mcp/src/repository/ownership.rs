//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the mcp
//! domain: moves every tool execution, artifact and MCP session from one user
//! to another in a single transaction. External session bindings carry a
//! provider credential hash minted for the old user and are deleted rather
//! than rebound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct McpOwnerReassignment;
    domain = "mcp";
    update "mcp_tool_executions" => "UPDATE mcp_tool_executions SET user_id = $2 WHERE user_id = $1";
    update "mcp_artifacts" => "UPDATE mcp_artifacts SET user_id = $2 WHERE user_id = $1";
    update "mcp_sessions" => "UPDATE mcp_sessions SET user_id = $2 WHERE user_id = $1";
    delete "mcp_external_sessions" => "DELETE FROM mcp_external_sessions WHERE user_id = $1";
}
