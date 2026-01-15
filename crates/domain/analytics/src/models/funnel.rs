use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{FunnelId, FunnelProgressId, SessionId};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Funnel {
    pub id: FunnelId,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FunnelStep {
    pub funnel_id: FunnelId,
    pub step_order: i32,
    pub name: String,
    pub match_pattern: String,
    pub match_type: FunnelMatchType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FunnelMatchType {
    UrlExact,
    UrlPrefix,
    UrlRegex,
    EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FunnelProgress {
    pub id: FunnelProgressId,
    pub funnel_id: FunnelId,
    pub session_id: SessionId,
    pub current_step: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub dropped_at_step: Option<i32>,
    pub step_timestamps: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFunnelInput {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<CreateFunnelStepInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFunnelStepInput {
    pub name: String,
    pub match_pattern: String,
    pub match_type: FunnelMatchType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelWithSteps {
    pub funnel: Funnel,
    pub steps: Vec<FunnelStep>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct FunnelStepStats {
    pub step_order: i32,
    pub entered_count: i64,
    pub exited_count: i64,
    pub conversion_rate: f64,
    pub avg_time_to_next_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelStats {
    pub funnel_id: FunnelId,
    pub funnel_name: String,
    pub total_entries: i64,
    pub total_completions: i64,
    pub overall_conversion_rate: f64,
    pub step_stats: Vec<FunnelStepStats>,
}
