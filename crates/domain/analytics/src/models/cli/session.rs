use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{SessionId, UserId};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct SessionStatsRow {
    pub total_sessions: i64,
    pub unique_users: i64,
    pub avg_duration: Option<f64>,
    pub avg_requests: Option<f64>,
    pub conversions: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ActiveSessionCountRow {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiveSessionRow {
    pub session_id: SessionId,
    pub user_type: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_seconds: Option<i32>,
    pub request_count: Option<i32>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionTrendRow {
    pub started_at: DateTime<Utc>,
    pub user_id: Option<UserId>,
    pub duration_seconds: Option<i32>,
}
