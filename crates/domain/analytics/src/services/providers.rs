//! Bridges this crate's analytics services to the `systemprompt_traits`
//! provider contracts.
//!
//! Implements [`AnalyticsProvider`] for `AnalyticsService` and
//! [`FingerprintProvider`] for `FingerprintRepository`, translating between
//! the crate-local types and the trait-level types and mapping every error
//! into the providers' error enums. `#[async_trait]` is required because
//! these provider traits are consumed as `dyn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::Utc;
use http::HeaderMap;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::{
    ActiveSession, AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, ExtractSignals, FingerprintProvider, SessionAnalytics,
};

use super::service::AnalyticsService;
use crate::repository::FingerprintRepository;

#[async_trait]
impl AnalyticsProvider for AnalyticsService {
    fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> SessionAnalytics {
        Self::extract_analytics(self, headers, signals)
    }

    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        self.create_analytics_session(input)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn find_recent_session_by_fingerprint(
        &self,
        fingerprint: &str,
        max_age_seconds: i64,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        let result = Self::find_recent_session_by_fingerprint(self, fingerprint, max_age_seconds)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))?;

        Ok(result.map(|r| AnalyticsSession {
            session_id: r.session_id,
            user_id: r.user_id,
            fingerprint: Some(fingerprint.to_owned()),
            created_at: Utc::now(),
        }))
    }

    async fn find_session_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<AnalyticsSession>> {
        let result = self
            .session_repo()
            .find_by_id(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))?;

        Ok(result.map(|r| AnalyticsSession {
            session_id: r.session_id,
            user_id: r.user_id,
            fingerprint: r.fingerprint_hash,
            created_at: r.started_at.unwrap_or_else(Utc::now),
        }))
    }

    async fn find_active_session_by_id(
        &self,
        session_id: &SessionId,
    ) -> AnalyticsResult<Option<ActiveSession>> {
        let result = self
            .session_repo()
            .find_active_by_id(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))?;

        Ok(result.map(|r| ActiveSession { user_id: r.user_id }))
    }

    async fn revoke_session(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        self.session_repo()
            .revoke_session(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn revoke_all_sessions_for_user(&self, user_id: &UserId) -> AnalyticsResult<u64> {
        self.session_repo()
            .revoke_all_for_user(user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn migrate_user_sessions(
        &self,
        from_user_id: &UserId,
        to_user_id: &UserId,
    ) -> AnalyticsResult<u64> {
        self.session_repo()
            .migrate_user_sessions(from_user_id, to_user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn mark_session_converted(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        self.session_repo()
            .mark_converted(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
}

#[async_trait]
impl FingerprintProvider for FingerprintRepository {
    async fn count_active_sessions(&self, fingerprint: &str) -> AnalyticsResult<i64> {
        self.count_active_sessions(fingerprint)
            .await
            .map(i64::from)
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn find_reusable_session(&self, fingerprint: &str) -> AnalyticsResult<Option<String>> {
        self.find_reusable_session(fingerprint)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn upsert_fingerprint(
        &self,
        fingerprint: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        _screen_info: Option<&str>,
    ) -> AnalyticsResult<()> {
        self.upsert_fingerprint(fingerprint, ip_address, user_agent, None)
            .await
            .map(|_| ())
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
}
