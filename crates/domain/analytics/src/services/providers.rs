//! Authoritative session persistence contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::service::AnalyticsService;
use crate::repository::FingerprintRepository;
use async_trait::async_trait;
use http::HeaderMap;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{
    AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, ExtractSignals,
    FingerprintProvider, SessionAnalytics,
};
impl AnalyticsProvider for AnalyticsService {
    fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> SessionAnalytics {
        Self::extract_analytics(self, headers, signals)
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

    async fn find_reusable_session(&self, fingerprint: &str) -> AnalyticsResult<Option<SessionId>> {
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
