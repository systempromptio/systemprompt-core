use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FingerprintReputation {
    pub fingerprint_hash: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub total_session_count: i32,
    pub active_session_count: i32,
    pub total_request_count: i64,
    pub requests_last_hour: i32,
    pub peak_requests_per_minute: f32,
    pub sustained_high_velocity_minutes: i32,
    pub is_flagged: bool,
    pub flag_reason: Option<String>,
    pub flagged_at: Option<DateTime<Utc>>,
    pub reputation_score: i32,
    pub abuse_incidents: i32,
    pub last_abuse_at: Option<DateTime<Utc>>,
    pub last_ip_address: Option<String>,
    pub last_user_agent: Option<String>,
    pub associated_user_ids: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagReason {
    HighRequestCount,
    SustainedVelocity,
    ExcessiveSessions,
    ReputationDecay,
}

impl FlagReason {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HighRequestCount => "request_count_exceeded_100",
            Self::SustainedVelocity => "sustained_velocity_10rpm_1hr",
            Self::ExcessiveSessions => "session_count_exceeded_10",
            Self::ReputationDecay => "reputation_score_below_threshold",
        }
    }
}

impl std::fmt::Display for FlagReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct FingerprintAnalysisResult {
    pub fingerprint_hash: String,
    pub should_flag: bool,
    pub flag_reasons: Vec<FlagReason>,
    pub new_reputation_score: i32,
    pub should_ban_ip: bool,
    pub ip_to_ban: Option<String>,
}
