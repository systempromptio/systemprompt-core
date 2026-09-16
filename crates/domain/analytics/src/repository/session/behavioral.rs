//! Behavioral analysis across typed owner contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{SessionBehavioralData, SessionRepository, behavioral_queries};
use crate::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::SessionId;
impl SessionRepository {
    pub async fn count_sessions_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_hours: i64,
    ) -> Result<i64> {
        systemprompt_traits::SessionStore::count_sessions_by_fingerprint(
            &*self.owner,
            fingerprint_hash,
            window_hours,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn get_endpoint_sequence(&self, session_id: &SessionId) -> Result<Vec<String>> {
        self.events
            .get_endpoint_sequence(session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn get_request_timestamps(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<DateTime<Utc>>> {
        self.events
            .get_request_timestamps(session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn get_total_content_pages(&self) -> Result<i64> {
        self.content.count_public_pages().await.map_err(Into::into)
    }

    pub async fn get_session_for_behavioral_analysis(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionBehavioralData>> {
        systemprompt_traits::SessionStore::get_session_for_behavioral_analysis(
            &*self.owner,
            session_id,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn has_analytics_events(&self, session_id: &SessionId) -> Result<bool> {
        self.events
            .has_analytics_events(session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn count_unique_ips_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<i64> {
        systemprompt_traits::SessionStore::count_unique_ips_by_fingerprint(
            &*self.owner,
            fingerprint_hash,
            window_days,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn count_engagement_events_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<i64> {
        let session_ids = self
            .owner
            .fingerprint_session_ids(fingerprint_hash, window_days)
            .await
            .map_err(crate::AnalyticsError::from)?
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        behavioral_queries::count_engagement_events_for_sessions(&self.write_pool, &session_ids)
            .await
    }

    pub async fn get_session_starts_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<Vec<DateTime<Utc>>> {
        systemprompt_traits::SessionStore::get_session_starts_by_fingerprint(
            &*self.owner,
            fingerprint_hash,
            window_days,
        )
        .await
        .map_err(crate::AnalyticsError::from)
    }

    pub async fn get_session_velocity(
        &self,
        session_id: &SessionId,
    ) -> Result<(Option<i64>, Option<i64>)> {
        systemprompt_traits::SessionStore::get_session_velocity(&*self.owner, session_id)
            .await
            .map_err(crate::AnalyticsError::from)
    }
}
