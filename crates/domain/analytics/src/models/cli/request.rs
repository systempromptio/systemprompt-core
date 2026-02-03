use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::AiRequestId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct RequestTrendRow {
    pub created_at: DateTime<Utc>,
    pub tokens_used: Option<i32>,
    pub cost_microdollars: Option<i64>,
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestListRow {
    pub id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: Option<i64>,
    pub latency_ms: Option<i32>,
    pub cache_hit: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
#[allow(clippy::struct_field_names)]
pub struct CostSummaryRow {
    pub total_requests: i64,
    pub total_cost: Option<i64>,
    pub total_tokens: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct CostTrendRow {
    pub created_at: DateTime<Utc>,
    pub cost_microdollars: Option<i64>,
    pub tokens_used: Option<i32>,
}
