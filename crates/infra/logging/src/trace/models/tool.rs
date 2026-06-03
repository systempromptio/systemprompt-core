//! MCP tool execution DTOs and audit linkage rows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{AiRequestId, ArtifactId, McpExecutionId, TaskId, TraceId};

#[derive(Debug, Clone)]
pub struct ToolExecutionFilter {
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub name: Option<String>,
    pub server: Option<String>,
    pub status: Option<String>,
}

impl ToolExecutionFilter {
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            since: None,
            name: None,
            server: None,
            status: None,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_name(name) -> String,
        with_server(server) -> String,
        with_status(status) -> String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionItem {
    pub timestamp: DateTime<Utc>,
    pub trace_id: TraceId,
    pub tool_name: String,
    pub server_name: Option<String>,
    pub status: String,
    pub execution_time_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLookupResult {
    pub id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub requested_model: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub task_id: Option<TaskId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditToolCallRow {
    pub tool_name: String,
    pub tool_input: String,
    pub sequence_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedMcpCall {
    pub tool_name: String,
    pub server_name: String,
    pub status: String,
    pub execution_time_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolExecution {
    pub mcp_execution_id: McpExecutionId,
    pub tool_name: String,
    pub server_name: String,
    pub status: String,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub input: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskArtifact {
    pub artifact_id: ArtifactId,
    pub artifact_type: String,
    pub name: Option<String>,
    pub source: Option<String>,
    pub tool_name: Option<String>,
    pub part_kind: Option<String>,
    pub text_content: Option<String>,
    pub data_content: Option<Value>,
}
