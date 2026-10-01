//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so rows decode into plain strings here and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AiToolCallId, ArtifactId, ContextId, McpExecutionId, McpServerId, McpToolName, SessionId,
    TraceId, UserId,
};

use crate::repository::McpArtifactRecord;

#[derive(Debug)]
pub(crate) struct McpArtifactRow {
    pub id: uuid::Uuid,
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub context_id: Option<ContextId>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub server_name: String,
    pub tool_name: Option<String>,
    pub artifact_type: String,
    pub title: Option<String>,
    pub source: String,
    pub last_seen_source: Option<String>,
    // JSON: the stored `ToolResponse` envelope, decoded as persisted.
    pub data: serde_json::Value,
    // JSON: `ExecutionMetadata` as persisted; the keyed columns are canonical.
    pub metadata: Option<serde_json::Value>,
    pub payload_sha256: Option<String>,
    pub payload_bytes: Option<i32>,
    pub is_structured: bool,
    pub has_ui_resource: bool,
    pub is_error: bool,
    pub secret_redactions: i32,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl From<McpArtifactRow> for McpArtifactRecord {
    fn from(row: McpArtifactRow) -> Self {
        Self {
            id: row.id,
            artifact_id: row.artifact_id,
            mcp_execution_id: row.mcp_execution_id,
            context_id: row.context_id,
            user_id: row.user_id,
            session_id: row.session_id,
            trace_id: row.trace_id,
            ai_tool_call_id: row.ai_tool_call_id,
            server_name: McpServerId::new(row.server_name),
            tool_name: row.tool_name.map(McpToolName::new),
            artifact_type: row.artifact_type,
            title: row.title,
            source: row.source,
            last_seen_source: row.last_seen_source,
            data: row.data,
            metadata: row.metadata,
            payload_sha256: row.payload_sha256,
            payload_bytes: row.payload_bytes,
            is_structured: row.is_structured,
            has_ui_resource: row.has_ui_resource,
            is_error: row.is_error,
            secret_redactions: row.secret_redactions,
            created_at: row.created_at,
            expires_at: row.expires_at,
        }
    }
}
