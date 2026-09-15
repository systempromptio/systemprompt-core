//! Authoritative session persistence contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SessionRepository;
use crate::Result;
use async_trait::async_trait;
use chrono::Utc;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::session_store::CreateSessionParams;
use systemprompt_traits::{
    ActiveSession, AnalyticsProviderError, AnalyticsResult, AnalyticsSession, CreateSessionInput,
    SessionProvider, SessionUsageCounters,
};
#[async_trait]
impl SessionProvider for SessionRepository {
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
        let result = self
            .find_recent_by_fingerprint(fingerprint, max_age_seconds)
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
            .find_active_by_id(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))?;

        Ok(result.map(|r| ActiveSession { user_id: r.user_id }))
    }

    async fn revoke_session(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        self.revoke_session(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn revoke_all_sessions_for_user(&self, user_id: &UserId) -> AnalyticsResult<u64> {
        self.revoke_all_for_user(user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn migrate_user_sessions(
        &self,
        from_user_id: &UserId,
        to_user_id: &UserId,
    ) -> AnalyticsResult<u64> {
        self.migrate_user_sessions(from_user_id, to_user_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn mark_session_converted(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        self.mark_converted(session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
}

#[async_trait]
impl SessionUsageCounters for SessionRepository {
    async fn increment_task_count(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::increment_task_count(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }

    async fn increment_message_count(&self, session_id: &SessionId) -> AnalyticsResult<()> {
        Self::increment_message_count(self, session_id)
            .await
            .map_err(|e| AnalyticsProviderError::Internal(e.to_string()))
    }
}

impl SessionRepository {
    pub async fn create_analytics_session(&self, input: CreateSessionInput<'_>) -> Result<()> {
        let fingerprint = input.analytics.compute_fingerprint();

        let params = CreateSessionParams {
            session_id: input.session_id,
            user_id: input.user_id,
            session_source: input.session_source,
            fingerprint_hash: Some(&fingerprint),
            ip_address: input.analytics.ip_address.as_deref(),
            user_agent: input.analytics.user_agent.as_deref(),
            device_type: input.analytics.device_type.as_deref(),
            browser: input.analytics.browser.as_deref(),
            os: input.analytics.os.as_deref(),
            country: input.analytics.country.as_deref(),
            region: input.analytics.region.as_deref(),
            city: input.analytics.city.as_deref(),
            preferred_locale: input.analytics.preferred_locale.as_deref(),
            referrer_source: input.analytics.referrer_source.as_deref(),
            referrer_url: input.analytics.referrer_url.as_deref(),
            landing_page: input.analytics.landing_page.as_deref(),
            entry_url: input.analytics.entry_url.as_deref(),
            utm_source: input.analytics.utm_source.as_deref(),
            utm_medium: input.analytics.utm_medium.as_deref(),
            utm_content: input.analytics.utm_content.as_deref(),
            utm_term: input.analytics.utm_term.as_deref(),
            utm_campaign: input.analytics.utm_campaign.as_deref(),
            is_bot: input.is_bot,
            is_ai_crawler: input.is_ai_crawler,
            expires_at: input.expires_at,
        };

        self.create_session(&params).await?;

        Ok(())
    }
}
