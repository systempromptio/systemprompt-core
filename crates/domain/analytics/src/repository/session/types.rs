use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, SessionSource, UserId};

#[derive(Debug)]
pub struct CreateSessionParams<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub session_source: SessionSource,
    pub fingerprint_hash: Option<&'a str>,
    pub ip_address: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub device_type: Option<&'a str>,
    pub browser: Option<&'a str>,
    pub os: Option<&'a str>,
    pub country: Option<&'a str>,
    pub region: Option<&'a str>,
    pub city: Option<&'a str>,
    pub preferred_locale: Option<&'a str>,
    pub referrer_source: Option<&'a str>,
    pub referrer_url: Option<&'a str>,
    pub landing_page: Option<&'a str>,
    pub entry_url: Option<&'a str>,
    pub utm_source: Option<&'a str>,
    pub utm_medium: Option<&'a str>,
    pub utm_campaign: Option<&'a str>,
    pub is_bot: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionBehavioralData {
    pub session_id: String,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub struct SessionMigrationResult {
    pub sessions_migrated: u64,
}

impl SessionMigrationResult {
    pub const fn total_records_migrated(&self) -> u64 {
        self.sessions_migrated
    }
}
