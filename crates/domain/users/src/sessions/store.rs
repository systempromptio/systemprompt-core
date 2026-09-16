//! Authoritative session persistence contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SessionRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::session_store::{
    ActiveSessionLookup, CreateSessionParams, SessionBehavioralData, SessionRecord, SessionSnapshot,
};
use systemprompt_traits::{AnalyticsProviderError, AnalyticsResult};
#[async_trait]
impl systemprompt_traits::SessionStore for SessionRepository {
    async fn fingerprint_session_ids(
        &self,
        fingerprint: &str,
        window_days: i64,
    ) -> AnalyticsResult<Vec<SessionId>> {
        Self::fingerprint_session_ids(self, fingerprint, window_days)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn find_by_id(&self, session_id: &SessionId) -> AnalyticsResult<Option<SessionSnapshot>> {
        Self::find_by_id(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn find_active_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<ActiveSessionLookup>> {
        Self::find_active_by_id(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn revoke_all_for_user(&self, user_id: &UserId) -> AnalyticsResult<u64> {
        Self::revoke_all_for_user(self, user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn find_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        user_id: &UserId,
    ) -> AnalyticsResult<Option<SessionSnapshot>> {
        Self::find_by_fingerprint(self, fingerprint_hash, user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn list_active_by_user(&self, user_id: &UserId) -> AnalyticsResult<Vec<SessionSnapshot>> {
        Self::list_active_by_user(self, user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn update_activity(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::update_activity(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn increment_request_count(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::increment_request_count(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn end_session(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::end_session(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn mark_as_scanner(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::mark_as_scanner(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn mark_converted(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::mark_converted(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn mark_as_behavioral_bot(
        &self,
        session_id: &SessionId,
        reason: &str,
    ) -> AnalyticsResult<()> {
        Self::mark_as_behavioral_bot(self, session_id, reason)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn check_and_mark_behavioral_bot(
        &self,
        session_id: &SessionId,
        request_count_threshold: i32,
    ) -> AnalyticsResult<bool> {
        Self::check_and_mark_behavioral_bot(self, session_id, request_count_threshold)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn cleanup_inactive(&self, inactive_hours: i32) -> AnalyticsResult<u64> {
        Self::cleanup_inactive(self, inactive_hours)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn count_inactive(&self, inactive_hours: i32) -> AnalyticsResult<i64> {
        Self::count_inactive(self, inactive_hours)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn count_sessions_missing_geo(&self) -> AnalyticsResult<i64> {
        Self::count_sessions_missing_geo(self)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn insert_session(&self, params: &CreateSessionParams<'_>) -> AnalyticsResult<()> {
        Self::create_session(self, params)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn find_recent_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<SessionRecord>> {
        Self::find_recent_by_fingerprint(self, fingerprint_hash, max_age_seconds)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AnalyticsResult<()> {
        Self::increment_ai_usage(self, session_id, tokens, cost_microdollars)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn update_behavioral_detection(
        &self,
        session_id: &SessionId,
        score: i32,
        is_behavioral_bot: bool,
        reason: Option<&str>,
    ) -> AnalyticsResult<()> {
        Self::update_behavioral_detection(self, session_id, score, is_behavioral_bot, reason)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn count_sessions_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_hours: i64,
    ) -> AnalyticsResult<i64> {
        Self::count_sessions_by_fingerprint(self, fingerprint_hash, window_hours)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn get_session_for_behavioral_analysis(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<SessionBehavioralData>> {
        Self::get_session_for_behavioral_analysis(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn count_unique_ips_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> AnalyticsResult<i64> {
        Self::count_unique_ips_by_fingerprint(self, fingerprint_hash, window_days)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn get_session_starts_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> AnalyticsResult<Vec<DateTime<Utc>>> {
        Self::get_session_starts_by_fingerprint(self, fingerprint_hash, window_days)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn get_session_velocity(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<(Option<i64>, Option<i64>)> {
        Self::get_session_velocity(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn count_active_fingerprint_sessions(&self, fingerprint: &str) -> AnalyticsResult<i32> {
        self.count_active_fingerprint(fingerprint)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn find_reusable_fingerprint_session(
        &self,
        fingerprint: &str,
    ) -> AnalyticsResult<Option<SessionId>> {
        self.find_reusable_fingerprint(fingerprint)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn sessions_missing_geo(
        &self,
        after: Option<&SessionId>,
        limit: i64,
    ) -> AnalyticsResult<Vec<(SessionId, String)>> {
        self.missing_geo(after, limit)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
    async fn set_session_geo(
        &self,
        session_id: &SessionId,
        country: Option<&str>,
        region: Option<&str>,
        city: Option<&str>,
    ) -> AnalyticsResult<u64> {
        self.set_geo(session_id, country, region, city)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
}
