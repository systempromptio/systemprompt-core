use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::UserId;

pub use systemprompt_models::ai as ai_models;

pub mod ai {
    pub use systemprompt_models::ai::*;
}

pub mod tools {
    pub use systemprompt_models::ai::tools::*;
}

pub mod ai_request_record;
pub mod image_generation;
pub mod message_converters;
pub mod providers;

pub use ai_request_record::{
    AiRequestRecord, AiRequestRecordBuilder, AiRequestRecordError, CacheInfo, RequestStatus,
    TokenInfo,
};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiRequest {
    pub id: String,
    pub request_id: String,
    pub user_id: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub context_id: Option<String>,
    pub trace_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i32>,
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_cents: i32,
    pub latency_ms: Option<i32>,
    pub cache_hit: bool,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub is_streaming: bool,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiRequestMessage {
    pub id: String,
    pub request_id: String,
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AiRequestToolCall {
    pub id: String,
    pub request_id: String,
    pub tool_name: String,
    pub tool_input: String,
    pub mcp_execution_id: Option<String>,
    pub sequence_number: i32,
    pub ai_tool_call_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProviderUsage {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAiUsage {
    #[sqlx(try_from = "String")]
    pub user_id: UserId,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_tokens_per_request: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostSummary {
    pub period: String,
    pub total_cost: f64,
    pub request_count: i64,
    pub avg_cost_per_request: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct LatencyPercentiles {
    #[sqlx(rename = "p50_ms")]
    #[serde(rename = "p50_ms")]
    pub p50: i64,
    #[sqlx(rename = "p90_ms")]
    #[serde(rename = "p90_ms")]
    pub p90: i64,
    #[sqlx(rename = "p95_ms")]
    #[serde(rename = "p95_ms")]
    pub p95: i64,
    #[sqlx(rename = "p99_ms")]
    #[serde(rename = "p99_ms")]
    pub p99: i64,
}
