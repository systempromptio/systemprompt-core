use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{AiToolCallId, ContextId, McpExecutionId, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Success,
    Failed,
}

impl ExecutionStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }

    pub const fn from_error(has_error: bool) -> Self {
        if has_error {
            Self::Failed
        } else {
            Self::Success
        }
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResultType {
    AuthRequired,
    PortUnavailable,
    ConnectionFailed,
    Timeout,
    Success,
    Error,
}

impl ValidationResultType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PortUnavailable => "port_unavailable",
            Self::ConnectionFailed => "connection_failed",
            Self::Timeout => "timeout",
            Self::Success => "success",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "auth_required" => Self::AuthRequired,
            "port_unavailable" => Self::PortUnavailable,
            "connection_failed" => Self::ConnectionFailed,
            "timeout" => Self::Timeout,
            "success" => Self::Success,
            _ => Self::Error,
        }
    }
}

impl std::fmt::Display for ValidationResultType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ToolExecutionRequest {
    pub tool_name: String,
    pub server_name: String,
    pub input: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub context: systemprompt_models::RequestContext,
    pub request_method: Option<String>,
    pub request_source: Option<String>,
    pub ai_tool_call_id: Option<AiToolCallId>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub output: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub status: String,
    pub error_message: Option<String>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MCPService {
    pub id: Uuid,
    pub name: String,
    pub module: String,
    pub port: i32,
    pub pid: Option<i32>,
    pub status: String,
    pub health: String,
    pub restart_count: i32,
    pub last_health_check: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const HEALTHY: &str = "healthy";

impl MCPService {
    pub fn is_running(&self) -> bool {
        self.status == crate::RUNNING
    }

    pub fn is_healthy(&self) -> bool {
        self.health == HEALTHY
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    pub mcp_execution_id: McpExecutionId,
    pub tool_name: String,
    pub server_name: String,
    pub context_id: Option<ContextId>,
    pub ai_tool_call_id: Option<AiToolCallId>,
    pub user_id: UserId,
    pub status: String,
    pub input: String,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub execution_time_ms: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolStats {
    pub tool_name: String,
    pub server_name: String,
    pub total_executions: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: Option<i64>,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
}
