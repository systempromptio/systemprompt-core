use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolListRow {
    pub tool_name: String,
    pub server_name: String,
    pub execution_count: i64,
    pub success_count: i64,
    pub avg_time: f64,
    pub last_used: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ToolStatsRow {
    pub total_tools: i64,
    pub total_executions: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ToolExistsRow {
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ToolSummaryRow {
    pub total: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolStatusBreakdownRow {
    pub status: String,
    pub status_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolErrorRow {
    pub error_msg: Option<String>,
    pub error_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolAgentUsageRow {
    pub agent_name: Option<String>,
    pub usage_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolExecutionRow {
    pub created_at: DateTime<Utc>,
    pub status: Option<String>,
    pub execution_time_ms: Option<i32>,
}
