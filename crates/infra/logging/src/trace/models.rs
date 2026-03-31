use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{
    ArtifactId, ContextId, McpExecutionId, SessionId, TaskId, TraceId, UserId,
};

#[derive(Debug, Clone)]
pub struct TraceListFilter {
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub tool: Option<String>,
    pub has_mcp: bool,
    pub include_system: bool,
}

impl TraceListFilter {
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            since: None,
            agent: None,
            status: None,
            tool: None,
            has_mcp: false,
            include_system: false,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_agent(agent) -> String,
        with_status(status) -> String,
        with_tool(tool) -> String,
    }

    pub const fn with_has_mcp(mut self, has_mcp: bool) -> Self {
        self.has_mcp = has_mcp;
        self
    }

    pub const fn with_include_system(mut self, include_system: bool) -> Self {
        self.include_system = include_system;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceListItem {
    pub trace_id: TraceId,
    pub first_timestamp: DateTime<Utc>,
    pub last_timestamp: DateTime<Utc>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub ai_requests: i64,
    pub mcp_calls: i64,
}

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

#[derive(Debug, Clone)]
pub struct LogSearchFilter {
    pub pattern: String,
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub level: Option<String>,
}

impl LogSearchFilter {
    pub const fn new(pattern: String, limit: i64) -> Self {
        Self {
            pattern,
            limit,
            since: None,
            level: None,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_level(level) -> String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSearchItem {
    pub id: String,
    pub trace_id: TraceId,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AiRequestFilter {
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

impl AiRequestFilter {
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            since: None,
            model: None,
            provider: None,
        }
    }

    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_model(model) -> String,
        with_provider(provider) -> String,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestListItem {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub trace_id: Option<TraceId>,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestDetail {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiRequestStats {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
    pub by_provider: Vec<ProviderStatsRow>,
    pub by_model: Vec<ModelStatsRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatsRow {
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatsRow {
    pub model: String,
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLookupResult {
    pub id: String,
    pub provider: String,
    pub model: String,
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
pub struct TraceEvent {
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub details: String,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub task_id: Option<TaskId>,
    pub context_id: Option<ContextId>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AiRequestSummary {
    pub total_cost_microdollars: i64,
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
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub agent_name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
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
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
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
    pub artifact_id: ArtifactId,
    pub artifact_type: String,
    pub name: Option<String>,
    pub source: Option<String>,
    pub tool_name: Option<String>,
    pub part_kind: Option<String>,
    pub text_content: Option<String>,
    pub data_content: Option<Value>,
}
