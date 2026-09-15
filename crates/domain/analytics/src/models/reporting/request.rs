//! CLI-facing request analytics rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
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

/// Selection for `RequestAnalyticsRepository::list_requests`: a page of
/// newest-first rows inside a window, optionally narrowed to one model
/// substring and one user.
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
    pub status: String,
    pub error_message: Option<String>,
    pub user_id: UserId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct CostSummaryRow {
    pub requests: i64,
    pub cost: Option<i64>,
    pub tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
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

/// Spend grouped by the user who made the requests. `name` is the display
/// name from the reporting projection (no email — that never leaves the
/// source table) and `conversations` counts distinct contexts, which is the
/// number that separates a user working through tasks from one sending many
/// one-line requests.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostUserBreakdownRow {
    pub user_id: String,
    pub name: Option<String>,
    pub cost: i64,
    pub requests: i64,
    pub tokens: i64,
    pub conversations: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ContextSummaryRow {
    pub conversations: i64,
    pub ai_requests: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContextGroupRow {
    pub name: String,
    pub conversations: i64,
    pub ai_requests: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecentContextRow {
    pub context_id: ContextId,
    pub last_activity: DateTime<Utc>,
    pub ai_requests: i64,
    pub model: Option<String>,
    pub agent_name: Option<String>,
    pub context_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct CostTrendRow {
    pub created_at: DateTime<Utc>,
    pub cost_microdollars: Option<i64>,
    pub tokens_used: Option<i32>,
}
