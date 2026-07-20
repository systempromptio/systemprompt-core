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
    CreateSessionInput, ExtractSignals, FingerprintProvider,
    SessionAnalytics as TraitSessionAnalytics,
};

use super::SessionAnalytics;
use super::service::AnalyticsService;
use crate::repository::FingerprintRepository;

#[async_trait]
impl AnalyticsProvider for AnalyticsService {
    fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> TraitSessionAnalytics {
        let local = Self::extract_analytics(self, headers, signals);
        TraitSessionAnalytics {
            ip_address: local.ip_address,
            user_agent: local.user_agent,
            device_type: local.device_type,
            browser: local.browser,
            os: local.os,
            fingerprint_hash: local.fingerprint_hash,
            referer: local.referrer_url.clone(),
            referrer_url: local.referrer_url,
            referrer_source: local.referrer_source,
            accept_language: local.preferred_locale.clone(),
            preferred_locale: local.preferred_locale,
            screen_width: None,
            screen_height: None,
            timezone: None,
            page_url: local.entry_url.clone(),
            landing_page: local.landing_page,
            entry_url: local.entry_url,
            country: local.country,
            region: local.region,
            city: local.city,
            utm_source: local.utm_source,
            utm_medium: local.utm_medium,
            utm_campaign: local.utm_campaign,
            utm_content: local.utm_content,
            utm_term: local.utm_term,
        }
    }

    async fn create_session(&self, input: CreateSessionInput<'_>) -> AnalyticsResult<()> {
        let local_analytics = SessionAnalytics {
            ip_address: input.analytics.ip_address.clone(),
            user_agent: input.analytics.user_agent.clone(),
            device_type: input.analytics.device_type.clone(),
            browser: input.analytics.browser.clone(),
            os: input.analytics.os.clone(),
            fingerprint_hash: input.analytics.fingerprint_hash.clone(),
            referrer_url: input
                .analytics
                .referrer_url
                .clone()
                .or_else(|| input.analytics.referer.clone()),
            referrer_source: input.analytics.referrer_source.clone(),
            preferred_locale: input
                .analytics
                .preferred_locale
                .clone()
                .or_else(|| input.analytics.accept_language.clone()),
            landing_page: input.analytics.landing_page.clone(),
            entry_url: input
                .analytics
                .entry_url
                .clone()
                .or_else(|| input.analytics.page_url.clone()),
            country: input.analytics.country.clone(),
            region: input.analytics.region.clone(),
            city: input.analytics.city.clone(),
            utm_source: input.analytics.utm_source.clone(),
            utm_medium: input.analytics.utm_medium.clone(),
            utm_campaign: input.analytics.utm_campaign.clone(),
            utm_content: input.analytics.utm_content.clone(),
            utm_term: input.analytics.utm_term.clone(),
        };

        let local_input = super::service::CreateAnalyticsSessionInput {
            session_id: input.session_id,
            user_id: input.user_id,
            analytics: &local_analytics,
            session_source: input.session_source,
            is_bot: input.is_bot,
            is_ai_crawler: input.is_ai_crawler,
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
