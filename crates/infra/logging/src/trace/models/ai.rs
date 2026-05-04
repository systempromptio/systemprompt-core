//! AI request DTOs: filters, list views, detail rows, aggregate stats, and
//! conversation messages.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, TraceId};

/// Filter parameters for listing AI requests.
#[derive(Debug, Clone)]
pub struct AiRequestFilter {
    pub limit: i64,
    pub since: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

impl AiRequestFilter {
    /// Construct a new AI request filter with the given row limit.
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            since: None,
            model: None,
            provider: None,
        }
    }

    /// Restrict results to requests issued at or after the given timestamp.
    pub const fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    systemprompt_models::builder_methods! {
        with_model(model) -> String,
        with_provider(provider) -> String,
    }
}

/// A row in the AI request list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestListItem {
    pub id: AiRequestId,
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

/// Detailed view of a single AI request, including any error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestDetail {
    pub id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

/// Aggregate AI request stats grouped by provider and model.
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

/// Per-provider rollup of AI request usage and cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatsRow {
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

/// Per-model rollup of AI request usage and cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatsRow {
    pub model: String,
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
    pub avg_latency_ms: i64,
}

/// Per-task AI request summary with token and latency totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequestInfo {
    pub id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub max_tokens: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
}

/// A single message in an AI request conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
}
