//! AI request DTOs: filters, list views, detail rows, aggregate stats, and
//! conversation messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, TraceId};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub sequence_number: i32,
}
