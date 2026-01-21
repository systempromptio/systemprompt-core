use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewConversationRow {
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewAgentRow {
    pub active_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewRequestRow {
    pub total: i64,
    pub total_tokens: Option<i64>,
    pub avg_latency: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewToolRow {
    pub total: i64,
    pub successful: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewActiveSessionRow {
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewTotalSessionRow {
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct OverviewCostRow {
    pub cost: Option<i64>,
}
