use async_trait::async_trait;
use chrono::Utc;
use http::{HeaderMap, Uri};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, FingerprintProvider, SessionAnalytics as TraitSessionAnalytics,
    SessionAnalyticsProvider, SessionAnalyticsProviderError, SessionAnalyticsResult,
};

use super::service::AnalyticsService;
use super::SessionAnalytics;
use crate::repository::{FingerprintRepository, SessionRepository};

#[async_trait]
impl AnalyticsProvider for AnalyticsService {
    fn extract_analytics(&self, headers: &HeaderMap, uri: Option<&Uri>) -> TraitSessionAnalytics {
        let local = Self::extract_analytics(self, headers, uri);
        TraitSessionAnalytics {
            ip_address: local.ip_address.clone(),
            user_agent: local.user_agent.clone(),
            referer: local.referrer_url.clone(),
            accept_language: local.preferred_locale.clone(),
            screen_width: None,
            screen_height: None,
            timezone: None,
            page_url: local.entry_url,
        }
    }

    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        let local_analytics = SessionAnalytics {
            ip_address: input.analytics.ip_address.clone(),
            user_agent: input.analytics.user_agent.clone(),
            referrer_url: input.analytics.referer.clone(),
            preferred_locale: input.analytics.accept_language.clone(),
            entry_url: input.analytics.page_url.clone(),
            ..Default::default()
        };

        let local_input = super::service::CreateAnalyticsSessionInput {
            session_id: input.session_id,
            user_id: input.user_id,
            analytics: &local_analytics,
            session_source: input.session_source,
            is_bot: input.is_bot,
            expires_at: input.expires_at,
        };

        self.create_analytics_session(local_input)
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
            session_id: r.session_id.to_string(),
            user_id: r.user_id.map(|u| u.to_string()),
            fingerprint: Some(fingerprint.to_string()),
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
            session_id: r.session_id.to_string(),
            user_id: r.user_id.map(|u| u.to_string()),
            fingerprint: r.fingerprint_hash,
            created_at: r.started_at.unwrap_or_else(Utc::now),
        }))
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

#[async_trait]
impl SessionAnalyticsProvider for SessionRepository {
    async fn increment_task_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()> {
        Self::increment_task_count(self, session_id)
            .await
            .map_err(|e| SessionAnalyticsProviderError::Internal(e.to_string()))
    }

    async fn increment_message_count(&self, session_id: &SessionId) -> SessionAnalyticsResult<()> {
        Self::increment_message_count(self, session_id)
            .await
            .map_err(|e| SessionAnalyticsProviderError::Internal(e.to_string()))
    }
}
