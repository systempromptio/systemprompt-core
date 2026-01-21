use std::sync::Arc;

use anyhow::Result;
use axum::extract::Request;
use axum::http::{HeaderMap, Uri};
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_models::ContentRouting;

use crate::repository::{CreateSessionParams, SessionRecord, SessionRepository};
use crate::services::SessionAnalytics;
use crate::GeoIpReader;

#[derive(Debug)]
pub struct CreateAnalyticsSessionInput<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub analytics: &'a SessionAnalytics,
    pub session_source: SessionSource,
    pub is_bot: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AnalyticsService {
    geoip_reader: Option<GeoIpReader>,
    content_routing: Option<Arc<dyn ContentRouting>>,
    session_repo: SessionRepository,
}

impl std::fmt::Debug for AnalyticsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsService")
            .field("geoip_reader", &self.geoip_reader.is_some())
            .field("content_routing", &self.content_routing.is_some())
            .field("session_repo", &"SessionRepository")
            .finish()
    }
}

impl AnalyticsService {
    pub fn new(
        db_pool: DbPool,
        geoip_reader: Option<GeoIpReader>,
        content_routing: Option<Arc<dyn ContentRouting>>,
    ) -> Self {
        Self {
            geoip_reader,
            content_routing,
            session_repo: SessionRepository::new(db_pool),
        }
    }

    pub fn extract_analytics(&self, headers: &HeaderMap, uri: Option<&Uri>) -> SessionAnalytics {
        SessionAnalytics::from_headers_and_uri(
            headers,
            uri,
            self.geoip_reader.as_ref(),
            self.content_routing.as_deref(),
        )
    }

    pub fn extract_from_request(&self, request: &Request) -> SessionAnalytics {
        SessionAnalytics::from_request(
            request,
            self.geoip_reader.as_ref(),
            self.content_routing.as_deref(),
        )
    }

    pub fn is_bot(analytics: &SessionAnalytics) -> bool {
        analytics.should_skip_tracking()
    }

    pub fn compute_fingerprint(analytics: &SessionAnalytics) -> String {
        analytics.fingerprint_hash.as_deref().map_or_else(
            || {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                analytics
                    .user_agent
                    .as_deref()
                    .unwrap_or("unknown")
                    .hash(&mut hasher);
                analytics
                    .preferred_locale
                    .as_deref()
                    .unwrap_or("")
                    .hash(&mut hasher);
                format!("{:x}", hasher.finish())
            },
            ToString::to_string,
        )
    }

    pub async fn create_analytics_session(
        &self,
        input: CreateAnalyticsSessionInput<'_>,
    ) -> Result<()> {
        let fingerprint = Self::compute_fingerprint(input.analytics);

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
            utm_campaign: input.analytics.utm_campaign.as_deref(),
            is_bot: input.is_bot,
            expires_at: input.expires_at,
        };

        self.session_repo.create_session(&params).await?;

        Ok(())
    }

    pub async fn find_recent_session_by_fingerprint(
        &self,
        fingerprint: &str,
        max_age_seconds: i64,
    ) -> Result<Option<SessionRecord>> {
        self.session_repo
            .find_recent_by_fingerprint(fingerprint, max_age_seconds)
            .await
    }

    pub const fn session_repo(&self) -> &SessionRepository {
        &self.session_repo
    }
}
