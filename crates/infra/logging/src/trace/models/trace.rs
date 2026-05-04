//! Trace-listing filters, events, and per-domain summaries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, ExecutionStepId, SessionId, TaskId, TraceId, UserId};

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
    pub step_id: ExecutionStepId,
    pub step_type: Option<String>,
    pub title: Option<String>,
    pub status: String,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
}
