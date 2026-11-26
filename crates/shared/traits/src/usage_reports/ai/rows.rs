//! Row and filter types the AI usage report port returns and accepts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, SessionId, UserId};

use crate::RequestStatus;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CostSummaryRow {
    pub requests: i64,
    pub cost: Option<i64>,
    pub tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PreviousCostRow {
    pub cost: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdownRow {
    pub name: String,
    pub cost: i64,
    pub requests: i64,
    pub tokens: i64,
}

/// Spend grouped by requesting user.
///
/// `name` is the user's display name; the email never leaves the source
/// table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostUserBreakdownRow {
    pub user_id: UserId,
    pub name: Option<String>,
    pub cost: i64,
    pub requests: i64,
    pub tokens: i64,
    pub conversations: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ContextSummaryRow {
    pub conversations: i64,
    pub ai_requests: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextGroupRow {
    pub name: String,
    pub conversations: i64,
    pub ai_requests: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CostTrendRow {
    pub created_at: DateTime<Utc>,
    pub cost_microdollars: Option<i64>,
    pub tokens_used: Option<i32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RequestStatsRow {
    pub total: i64,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cost: Option<i64>,
    pub avg_latency: Option<f64>,
    pub cache_hits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageRow {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub total_tokens: Option<i64>,
    pub total_cost: Option<i64>,
    pub avg_latency: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RequestTrendRow {
    pub created_at: DateTime<Utc>,
    pub tokens_used: Option<i32>,
    pub cost_microdollars: Option<i64>,
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct RequestListFilter {
    pub limit: i64,
    pub offset: i64,
    pub model: Option<String>,
    pub user: Option<UserId>,
}

impl RequestListFilter {
    #[must_use]
    pub const fn new(limit: i64) -> Self {
        Self {
            limit,
            offset: 0,
            model: None,
            user: None,
        }
    }

    #[must_use]
    pub const fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    #[must_use]
    pub fn with_user(mut self, user: UserId) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestListRow {
    pub id: AiRequestId,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_microdollars: Option<i64>,
    pub latency_ms: Option<i32>,
    pub cache_hit: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub status: RequestStatus,
    pub error_message: Option<String>,
    pub user_id: UserId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RequestMetricsRow {
    pub total: i64,
    pub total_tokens: Option<i64>,
    pub avg_latency: Option<f64>,
}

/// One gateway session: AI requests that carry a session and no task.
///
/// Sessions of agent contexts are included; a report that lists them apart
/// filters them out with the agent member's context list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySessionListRow {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
