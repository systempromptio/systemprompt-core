
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// =============================================================================
// Agent Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentListRow {
    pub agent_name: String,
    pub task_count: i64,
    pub completed_count: i64,
    pub avg_execution_time_ms: i64,
    pub total_cost_cents: i64,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentStatsRow {
    pub total_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub failed_tasks: i64,
    pub avg_execution_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentHourlyRow {
    pub task_hour: i32,
    pub task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentSummaryRow {
    pub total_tasks: i64,
    pub completed: i64,
    pub failed: i64,
    pub avg_time: f64,
}

// =============================================================================
// Conversation Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationListRow {
    pub context_id: String,
    pub name: Option<String>,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationStatsRow {
    pub total_contexts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskStatsRow {
    pub total_tasks: i64,
    pub avg_execution_time_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MessageCountRow {
    pub total_messages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimestampRow {
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// Session Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionStatsRow {
    pub total_sessions: i64,
    pub unique_users: i64,
    pub avg_duration: Option<f64>,
    pub avg_requests: Option<f64>,
    pub conversions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActiveSessionCountRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiveSessionRow {
    pub session_id: String,
    pub user_type: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_seconds: Option<i32>,
    pub request_count: Option<i32>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionTrendRow {
    pub started_at: DateTime<Utc>,
    pub user_id: Option<String>,
    pub duration_seconds: Option<i32>,
}

// =============================================================================
// Tool Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolListRow {
    pub tool_name: String,
    pub server_name: String,
    pub execution_count: i64,
    pub success_count: i64,
    pub avg_time: f64,
    pub last_used: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolStatsRow {
    pub total_tools: i64,
    pub total_executions: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolExistsRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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

// =============================================================================
// Request Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestStatsRow {
    pub total: i64,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cost: Option<i64>,
    pub avg_latency: Option<f64>,
    pub cache_hits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModelUsageRow {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub total_tokens: Option<i64>,
    pub total_cost: Option<i64>,
    pub avg_latency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestTrendRow {
    pub created_at: DateTime<Utc>,
    pub tokens_used: Option<i32>,
    pub cost_cents: Option<i32>,
    pub latency_ms: Option<i32>,
}

// =============================================================================
// Cost Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[allow(clippy::struct_field_names)]
pub struct CostSummaryRow {
    pub total_requests: i64,
    pub total_cost: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PreviousCostRow {
    pub cost: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostBreakdownRow {
    pub name: String,
    pub cost: i64,
    pub requests: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostTrendRow {
    pub created_at: DateTime<Utc>,
    pub cost_cents: Option<i32>,
    pub tokens_used: Option<i32>,
}

// =============================================================================
// Content Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TopContentRow {
    pub content_id: String,
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: Option<f64>,
    pub trend_direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContentStatsRow {
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_time_on_page_seconds: Option<f64>,
    pub avg_scroll_depth: Option<f64>,
    pub total_clicks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContentTrendRow {
    pub timestamp: DateTime<Utc>,
    pub views: i64,
    pub unique_visitors: i64,
}

// =============================================================================
// Traffic Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrafficSourceRow {
    pub source: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GeoRow {
    pub country: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceRow {
    pub device: Option<String>,
    pub browser: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BotTotalsRow {
    pub human: i64,
    pub bot: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BotTypeRow {
    pub bot_type: Option<String>,
    pub count: i64,
}

// =============================================================================
// Overview Analytics Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewConversationRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewAgentRow {
    pub active_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewRequestRow {
    pub total: i64,
    pub total_tokens: Option<i64>,
    pub avg_latency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewToolRow {
    pub total: i64,
    pub successful: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewActiveSessionRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewTotalSessionRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverviewCostRow {
    pub cost: Option<i64>,
}
