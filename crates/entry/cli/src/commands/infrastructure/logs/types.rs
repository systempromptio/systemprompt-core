//! Log-command argument and row types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpServerId, McpToolName};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageRow {
    pub sequence: i32,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallRow {
    pub tool_name: McpToolName,
    pub server: McpServerId,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}
