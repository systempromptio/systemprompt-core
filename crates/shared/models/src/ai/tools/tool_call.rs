//! Provider tool-call request type.
//!
//! [`ToolCall`] is a requested invocation (id, name, arguments);
//! [`CallToolResult`] is the MCP result it resolves to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub ai_tool_call_id: AiToolCallId,
    pub name: String,
    // JSON: MCP tool-call arguments / result are the tool's own JSON.
    pub arguments: JsonValue,
}

pub use rmcp::model::CallToolResult;
