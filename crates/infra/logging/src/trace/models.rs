use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub details: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub context_id: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AiRequestSummary {
    pub total_cost_cents: i64,
    pub total_tokens: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub request_count: i64,
    pub total_latency_ms: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct McpExecutionSummary {
    pub execution_count: i64,
    pub total_execution_time_ms: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ExecutionStepSummary {
    #[serde(rename = "step_count")]
    pub total: i64,
    #[serde(rename = "completed_count")]
    pub completed: i64,
    #[serde(rename = "failed_count")]
    pub failed: i64,
    #[serde(rename = "pending_count")]
    pub pending: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub task_id: String,
    pub context_id: String,
    pub agent_name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_id: String,
    pub step_type: Option<String>,
    pub title: Option<String>,
    pub status: String,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestInfo {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub max_tokens: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_cents: i32,
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolExecution {
    pub mcp_execution_id: String,
    pub tool_name: String,
    pub server_name: String,
    pub status: String,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub input: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
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
    pub artifact_id: String,
    pub artifact_type: String,
    pub name: Option<String>,
    pub source: Option<String>,
    pub tool_name: Option<String>,
    pub part_kind: Option<String>,
    pub text_content: Option<String>,
    pub data_content: Option<Value>,
}
