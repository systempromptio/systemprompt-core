mod behavioral;
mod mutations;
mod queries;
mod types;

use anyhow::Result;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};

use crate::models::AnalyticsSession;

pub use types::{
    CreateSessionParams, SessionBehavioralData, SessionMigrationResult, SessionRecord,
};

#[derive(Clone, Debug)]
pub struct SessionRepository {
    pool: DbPool,
}

impl SessionRepository {
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, session_id: &SessionId) -> Result<Option<AnalyticsSession>> {
        queries::find_by_id(&self.pool, session_id).await
    }

    pub async fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        user_id: &UserId,
    ) -> Result<Option<AnalyticsSession>> {
        queries::find_by_fingerprint(&self.pool, fingerprint_hash, user_id).await
    }

    pub async fn list_active_by_user(&self, user_id: &UserId) -> Result<Vec<AnalyticsSession>> {
        queries::list_active_by_user(&self.pool, user_id).await
    }

    pub async fn update_activity(&self, session_id: &SessionId) -> Result<()> {
        mutations::update_activity(&self.pool, session_id).await
    }

    pub async fn increment_request_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_request_count(&self.pool, session_id).await
    }

    pub async fn increment_task_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_task_count(&self.pool, session_id).await
    }

    pub async fn increment_ai_request_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_ai_request_count(&self.pool, session_id).await
    }

    pub async fn increment_message_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_message_count(&self.pool, session_id).await
    }

    pub async fn end_session(&self, session_id: &SessionId) -> Result<()> {
        mutations::end_session(&self.pool, session_id).await
    }

    pub async fn mark_as_scanner(&self, session_id: &SessionId) -> Result<()> {
        mutations::mark_as_scanner(&self.pool, session_id).await
    }

    pub async fn mark_as_behavioral_bot(&self, session_id: &SessionId, reason: &str) -> Result<()> {
        behavioral::mark_as_behavioral_bot(&self.pool, session_id, reason).await
    }

    pub async fn check_and_mark_behavioral_bot(
        &self,
        session_id: &SessionId,
        request_count_threshold: i32,
    ) -> Result<bool> {
        behavioral::check_and_mark_behavioral_bot(&self.pool, session_id, request_count_threshold)
            .await
    }

    pub async fn cleanup_inactive(&self, inactive_hours: i32) -> Result<u64> {
        mutations::cleanup_inactive(&self.pool, inactive_hours).await
    }

    pub async fn migrate_user_sessions(
        &self,
        old_user_id: &UserId,
        new_user_id: &UserId,
    ) -> Result<u64> {
        mutations::migrate_user_sessions(&self.pool, old_user_id, new_user_id).await
    }

    pub async fn create_session(&self, params: &CreateSessionParams<'_>) -> Result<()> {
        mutations::create_session(&self.pool, params).await
    }

    pub async fn find_recent_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        max_age_seconds: i64,
    ) -> Result<Option<SessionRecord>> {
        queries::find_recent_by_fingerprint(&self.pool, fingerprint_hash, max_age_seconds).await
    }

    pub async fn exists(&self, session_id: &SessionId) -> Result<bool> {
        queries::exists(&self.pool, session_id).await
    }

    pub async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_cents: i32,
    ) -> Result<()> {
        mutations::increment_ai_usage(&self.pool, session_id, tokens, cost_cents).await
    }

    pub async fn update_behavioral_detection(
        &self,
        session_id: &SessionId,
        score: i32,
        is_behavioral_bot: bool,
        reason: Option<&str>,
    ) -> Result<()> {
        behavioral::update_behavioral_detection(
            &self.pool,
            session_id,
            score,
            is_behavioral_bot,
            reason,
        )
        .await
    }

    pub async fn escalate_throttle(&self, session_id: &SessionId, new_level: i32) -> Result<()> {
        mutations::escalate_throttle(&self.pool, session_id, new_level).await
    }

    pub async fn get_throttle_level(&self, session_id: &SessionId) -> Result<i32> {
        queries::get_throttle_level(&self.pool, session_id).await
    }

    pub async fn count_sessions_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_hours: i64,
    ) -> Result<i64> {
        queries::count_sessions_by_fingerprint(&self.pool, fingerprint_hash, window_hours).await
    }

    pub async fn get_endpoint_sequence(&self, session_id: &SessionId) -> Result<Vec<String>> {
        queries::get_endpoint_sequence(&self.pool, session_id).await
    }

    pub async fn get_request_timestamps(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<DateTime<Utc>>> {
        queries::get_request_timestamps(&self.pool, session_id).await
    }

    pub async fn get_total_content_pages(&self) -> Result<i64> {
        queries::get_total_content_pages(&self.pool).await
    }

    pub async fn get_session_for_behavioral_analysis(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionBehavioralData>> {
        queries::get_session_for_behavioral_analysis(&self.pool, session_id).await
    }
}
