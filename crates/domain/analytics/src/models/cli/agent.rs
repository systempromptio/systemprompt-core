use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ContextId;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentListRow {
    pub agent_name: String,
    pub task_count: i64,
    pub completed_count: i64,
    pub avg_execution_time_ms: i64,
    pub total_cost_cents: i64,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct AgentStatsRow {
    pub total_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub failed_tasks: i64,
    pub avg_execution_time_ms: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct AgentAiStatsRow {
    pub total_ai_requests: i64,
    pub total_cost_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentTaskRow {
    pub started_at: DateTime<Utc>,
    pub status: Option<String>,
    pub execution_time_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentStatusBreakdownRow {
    pub status: String,
    pub status_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentErrorRow {
    pub error_type: Option<String>,
    pub error_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct AgentHourlyRow {
    pub task_hour: i32,
    pub task_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct AgentSummaryRow {
    pub total_tasks: i64,
    pub completed: i64,
    pub failed: i64,
    pub avg_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationListRow {
    pub context_id: ContextId,
    pub name: Option<String>,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ConversationStatsRow {
    pub total_contexts: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct TaskStatsRow {
    pub total_tasks: i64,
    pub avg_execution_time_ms: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct MessageCountRow {
    pub total_messages: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct TimestampRow {
    pub timestamp: DateTime<Utc>,
}
