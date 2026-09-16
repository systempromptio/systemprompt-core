//! Session analytics orchestration through authoritative owner contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod behavioral;
mod behavioral_queries;
mod geo;
mod types;

use std::sync::Arc;

use crate::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};

use crate::models::AnalyticsSession;

use systemprompt_traits::session_store::ActiveSessionLookup;
pub use types::{
    CreateSessionParams, SessionBehavioralData, SessionMigrationResult, SessionRecord,
};

#[derive(Clone)]
pub struct SessionRepository {
    write_pool: Arc<PgPool>,
    owner: systemprompt_traits::DynSessionStore,
    events: systemprompt_traits::DynAnalyticsEventStore,
    content: systemprompt_traits::DynContentCatalogStats,
}

impl SessionRepository {
    pub fn owner(&self) -> systemprompt_traits::DynSessionStore {
        Arc::clone(&self.owner)
    }

    pub fn new(
        db: &DbPool,
        owner: systemprompt_traits::DynSessionStore,
        events: systemprompt_traits::DynAnalyticsEventStore,
        content: systemprompt_traits::DynContentCatalogStats,
    ) -> Result<Self> {
        let write_pool = db.write_pool_arc()?;
        Ok(Self {
            write_pool,
            owner,
            events,
            content,
        })
    }

    pub async fn find_by_id(&self, session_id: &SessionId) -> Result<Option<AnalyticsSession>> {
        systemprompt_traits::SessionStore::find_by_id(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn find_active_by_id(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ActiveSessionLookup>> {
        systemprompt_traits::SessionStore::find_active_by_id(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn revoke_session(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionProvider::revoke_session(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn revoke_all_for_user(&self, user_id: &UserId) -> Result<u64> {
        systemprompt_traits::SessionStore::revoke_all_for_user(&*self.owner, user_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        user_id: &UserId,
    ) -> Result<Option<AnalyticsSession>> {
        systemprompt_traits::SessionStore::find_by_fingerprint(
            &*self.owner,
            fingerprint_hash,
            user_id,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn list_active_by_user(&self, user_id: &UserId) -> Result<Vec<AnalyticsSession>> {
        systemprompt_traits::SessionStore::list_active_by_user(&*self.owner, user_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn update_activity(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionStore::update_activity(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn increment_request_count(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionStore::increment_request_count(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn increment_task_count(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionUsageCounters::increment_task_count(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn increment_message_count(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionUsageCounters::increment_message_count(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn end_session(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionStore::end_session(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn mark_as_scanner(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionStore::mark_as_scanner(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn mark_converted(&self, session_id: &SessionId) -> Result<()> {
        systemprompt_traits::SessionStore::mark_converted(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn mark_as_behavioral_bot(&self, session_id: &SessionId, reason: &str) -> Result<()> {
        systemprompt_traits::SessionStore::mark_as_behavioral_bot(&*self.owner, session_id, reason)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn check_and_mark_behavioral_bot(
        &self,
        session_id: &SessionId,
        request_count_threshold: i32,
    ) -> Result<bool> {
        systemprompt_traits::SessionStore::check_and_mark_behavioral_bot(
            &*self.owner,
            session_id,
            request_count_threshold,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn cleanup_inactive(&self, inactive_hours: i32) -> Result<u64> {
        systemprompt_traits::SessionStore::cleanup_inactive(&*self.owner, inactive_hours)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn count_inactive(&self, inactive_hours: i32) -> Result<i64> {
        systemprompt_traits::SessionStore::count_inactive(&*self.owner, inactive_hours)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn backfill_session_geo(
        &self,
        geoip_reader: Option<&crate::GeoIpReader>,
        batch_size: i64,
    ) -> Result<u64> {
        self.backfill_geo(geoip_reader, batch_size).await
    }

    pub async fn count_sessions_missing_geo(&self) -> Result<i64> {
        systemprompt_traits::SessionStore::count_sessions_missing_geo(&*self.owner)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn migrate_user_sessions(
        &self,
        old_user_id: &UserId,
        new_user_id: &UserId,
    ) -> Result<u64> {
        systemprompt_traits::SessionProvider::migrate_user_sessions(
            &*self.owner,
            old_user_id,
            new_user_id,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn create_session(&self, params: &CreateSessionParams<'_>) -> Result<()> {
        systemprompt_traits::SessionStore::insert_session(&*self.owner, params)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn find_recent_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        max_age_seconds: i64,
    ) -> Result<Option<SessionRecord>> {
        systemprompt_traits::SessionStore::find_recent_by_fingerprint(
            &*self.owner,
            fingerprint_hash,
            max_age_seconds,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> Result<()> {
        systemprompt_traits::SessionStore::increment_ai_usage(
            &*self.owner,
            session_id,
            tokens,
            cost_microdollars,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn update_behavioral_detection(
        &self,
        session_id: &SessionId,
        score: i32,
        is_behavioral_bot: bool,
        reason: Option<&str>,
    ) -> Result<()> {
        systemprompt_traits::SessionStore::update_behavioral_detection(
            &*self.owner,
            session_id,
            score,
            is_behavioral_bot,
            reason,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }
}

impl std::fmt::Debug for SessionRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRepository").finish_non_exhaustive()
    }
}
