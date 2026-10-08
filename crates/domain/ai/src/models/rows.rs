//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so rows decode into plain strings here and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, McpExecutionId, McpToolName};

use super::AiRequestToolCall;

#[derive(Debug)]
pub(crate) struct AiRequestToolCallRow {
    pub id: String,
    pub request_id: AiRequestId,
    pub tool_name: String,
    pub tool_input: String,
    pub mcp_execution_id: Option<McpExecutionId>,
    pub sequence_number: i32,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiRequestToolCallRow> for AiRequestToolCall {
    fn from(row: AiRequestToolCallRow) -> Self {
        Self {
            id: row.id,
            request_id: row.request_id,
            tool_name: McpToolName::new(row.tool_name),
            tool_input: row.tool_input,
            mcp_execution_id: row.mcp_execution_id,
            sequence_number: row.sequence_number,
            ai_tool_call_id: row.ai_tool_call_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
