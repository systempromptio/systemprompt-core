//! Session repository row and migration-result types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    pub utm_content: Option<&'a str>,
    pub utm_term: Option<&'a str>,
    pub is_bot: bool,
    pub is_ai_crawler: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ActiveSessionLookup {
    pub user_id: Option<UserId>,
}

#[derive(Debug, Clone)]
pub struct SessionBehavioralData {
    pub session_id: SessionId,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub landing_page: Option<String>,
    pub entry_url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub fingerprint_hash: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub referrer_url: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub utm_term: Option<String>,
    pub is_bot: bool,
    pub is_scanner: Option<bool>,
    pub is_behavioral_bot: Option<bool>,
    pub behavioral_bot_reason: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub request_count: Option<i32>,
    pub task_count: Option<i32>,
    pub ai_request_count: Option<i32>,
    pub message_count: Option<i32>,
}

use crate::{AnalyticsResult, SessionProvider, SessionUsageCounters};
use async_trait::async_trait;

/// Users-owned session persistence injected into analytics as
/// `dyn SessionStore`, hence `#[async_trait]`.
#[async_trait]
pub trait SessionStore: SessionProvider + SessionUsageCounters {
    async fn fingerprint_session_ids(
        &self,
        fingerprint: &str,
        window_days: i64,
    ) -> AnalyticsResult<Vec<SessionId>>;
    async fn find_by_id(&self, session_id: &SessionId) -> AnalyticsResult<Option<SessionSnapshot>>;
    async fn find_active_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<ActiveSessionLookup>>;
    async fn revoke_all_for_user(&self, user_id: &UserId) -> AnalyticsResult<u64>;
    async fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        user_id: &UserId,
    ) -> AnalyticsResult<Option<SessionSnapshot>>;
    async fn list_active_by_user(&self, user_id: &UserId) -> AnalyticsResult<Vec<SessionSnapshot>>;
    async fn update_activity(&self, session_id: &SessionId) -> AnalyticsResult<()>;
    async fn increment_request_count(&self, session_id: &SessionId) -> AnalyticsResult<()>;
    async fn end_session(&self, session_id: &SessionId) -> AnalyticsResult<()>;
    async fn mark_as_scanner(&self, session_id: &SessionId) -> AnalyticsResult<()>;
    async fn mark_converted(&self, session_id: &SessionId) -> AnalyticsResult<()>;
    async fn mark_as_behavioral_bot(
        &self,
        session_id: &SessionId,
        reason: &str,
    ) -> AnalyticsResult<()>;
    async fn check_and_mark_behavioral_bot(
        &self,
        session_id: &SessionId,
        request_count_threshold: i32,
    ) -> AnalyticsResult<bool>;
    async fn cleanup_inactive(&self, inactive_hours: i32) -> AnalyticsResult<u64>;
    async fn count_inactive(&self, inactive_hours: i32) -> AnalyticsResult<i64>;
    async fn count_sessions_missing_geo(&self) -> AnalyticsResult<i64>;
    async fn insert_session(&self, params: &CreateSessionParams<'_>) -> AnalyticsResult<()>;
    async fn find_recent_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<SessionRecord>>;
    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AnalyticsResult<()>;
    async fn update_behavioral_detection(
        &self,
        session_id: &SessionId,
        score: i32,
        is_behavioral_bot: bool,
        reason: Option<&str>,
    ) -> AnalyticsResult<()>;
    async fn count_sessions_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_hours: i64,
    ) -> AnalyticsResult<i64>;
    async fn get_session_for_behavioral_analysis(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<SessionBehavioralData>>;
    async fn count_unique_ips_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> AnalyticsResult<i64>;
    async fn get_session_starts_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> AnalyticsResult<Vec<DateTime<Utc>>>;
    async fn get_session_velocity(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<(Option<i64>, Option<i64>)>;
    async fn count_active_fingerprint_sessions(&self, fingerprint: &str) -> AnalyticsResult<i32>;
    async fn find_reusable_fingerprint_session(
        &self,
        fingerprint: &str,
    ) -> AnalyticsResult<Option<SessionId>>;
    async fn sessions_missing_geo(
        &self,
        after: Option<&SessionId>,
        limit: i64,
    ) -> AnalyticsResult<Vec<(SessionId, String)>>;
    async fn set_session_geo(
        &self,
        session_id: &SessionId,
        country: Option<&str>,
        region: Option<&str>,
        city: Option<&str>,
    ) -> AnalyticsResult<u64>;
}
pub type DynSessionStore = std::sync::Arc<dyn SessionStore>;
