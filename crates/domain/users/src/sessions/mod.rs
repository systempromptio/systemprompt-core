//! Authoritative session persistence contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod behavioral;
mod behavioral_queries;
mod geo;
mod mutations;
mod providers;
mod queries;
use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::session_store::{
    ActiveSessionLookup, CreateSessionParams, SessionBehavioralData, SessionRecord,
    SessionSnapshot as AnalyticsSession,
};
#[derive(Clone, Debug)]
pub struct SessionRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}
impl SessionRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.pool_arc()?,
            write_pool: db.write_pool_arc()?,
        })
    }
    pub async fn find_by_id(&self, session_id: &SessionId) -> Result<Option<AnalyticsSession>> {
        queries::find_by_id(&self.write_pool, session_id).await
    }
    pub async fn find_active_by_id(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ActiveSessionLookup>> {
        queries::find_active_by_id(&self.write_pool, session_id).await
    }
    pub async fn revoke_session(&self, session_id: &SessionId) -> Result<()> {
        mutations::revoke_session(&self.write_pool, session_id).await
    }
    pub async fn revoke_all_for_user(&self, user_id: &UserId) -> Result<u64> {
        mutations::revoke_all_for_user(&self.write_pool, user_id).await
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
        mutations::update_activity(&self.write_pool, session_id).await
    }
    pub async fn increment_request_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_request_count(&self.write_pool, session_id).await
    }
    pub async fn increment_task_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_task_count(&self.write_pool, session_id).await
    }
    pub async fn increment_message_count(&self, session_id: &SessionId) -> Result<()> {
        mutations::increment_message_count(&self.write_pool, session_id).await
    }
    pub async fn end_session(&self, session_id: &SessionId) -> Result<()> {
        mutations::end_session(&self.write_pool, session_id).await
    }
    pub async fn mark_as_scanner(&self, session_id: &SessionId) -> Result<()> {
        mutations::mark_as_scanner(&self.write_pool, session_id).await
    }
    pub async fn mark_converted(&self, session_id: &SessionId) -> Result<()> {
        mutations::mark_converted(&self.write_pool, session_id).await
    }
    pub async fn mark_as_behavioral_bot(&self, session_id: &SessionId, reason: &str) -> Result<()> {
        behavioral::mark_as_behavioral_bot(&self.write_pool, session_id, reason).await
    }
    pub async fn check_and_mark_behavioral_bot(
        &self,
        session_id: &SessionId,
        request_count_threshold: i32,
    ) -> Result<bool> {
        behavioral::check_and_mark_behavioral_bot(
            &self.write_pool,
            session_id,
            request_count_threshold,
        )
        .await
    }
    pub async fn cleanup_inactive(&self, inactive_hours: i32) -> Result<u64> {
        mutations::cleanup_inactive(&self.write_pool, inactive_hours).await
    }
    pub async fn count_inactive(&self, inactive_hours: i32) -> Result<i64> {
        queries::count_inactive(&self.pool, inactive_hours).await
    }
    pub async fn count_sessions_missing_geo(&self) -> Result<i64> {
        mutations::count_sessions_missing_geo(&self.pool).await
    }
    pub async fn migrate_user_sessions(
        &self,
        old_user_id: &UserId,
        new_user_id: &UserId,
    ) -> Result<u64> {
        mutations::migrate_user_sessions(&self.write_pool, old_user_id, new_user_id).await
    }
    pub async fn create_session(&self, params: &CreateSessionParams<'_>) -> Result<()> {
        mutations::create_session(&self.write_pool, params).await
    }
    pub async fn find_recent_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        max_age_seconds: i64,
    ) -> Result<Option<SessionRecord>> {
        queries::find_recent_by_fingerprint(&self.write_pool, fingerprint_hash, max_age_seconds)
            .await
    }
    pub async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> Result<()> {
        mutations::increment_ai_usage(&self.write_pool, session_id, tokens, cost_microdollars).await
    }
    pub async fn update_behavioral_detection(
        &self,
        session_id: &SessionId,
        score: i32,
        is_behavioral_bot: bool,
        reason: Option<&str>,
    ) -> Result<()> {
        behavioral::update_behavioral_detection(
            &self.write_pool,
            session_id,
            score,
            is_behavioral_bot,
            reason,
        )
        .await
    }
    pub async fn count_sessions_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_hours: i64,
    ) -> Result<i64> {
        behavioral_queries::count_sessions_by_fingerprint(
            &self.write_pool,
            fingerprint_hash,
            window_hours,
        )
        .await
    }
    pub async fn get_session_for_behavioral_analysis(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionBehavioralData>> {
        behavioral_queries::get_session_for_behavioral_analysis(&self.write_pool, session_id).await
    }
    pub async fn count_unique_ips_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<i64> {
        behavioral_queries::count_unique_ips_by_fingerprint(
            &self.write_pool,
            fingerprint_hash,
            window_days,
        )
        .await
    }
    pub async fn get_session_starts_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<Vec<DateTime<Utc>>> {
        behavioral_queries::get_session_starts_by_fingerprint(
            &self.write_pool,
            fingerprint_hash,
            window_days,
        )
        .await
    }
    pub async fn get_session_velocity(
        &self,
        session_id: &SessionId,
    ) -> Result<(Option<i64>, Option<i64>)> {
        behavioral_queries::get_session_velocity(&self.write_pool, session_id).await
    }
}

mod ai_provider;
pub use ai_provider::UsersAiSessionProvider;

mod fingerprint;

mod store;
