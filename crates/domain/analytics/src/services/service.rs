//! `AnalyticsService`: request/session recording facade over the repositories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::Result;
use chrono::{DateTime, Utc};
use http::HeaderMap;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_models::ContentRouting;
use systemprompt_traits::ExtractSignals;

use crate::GeoIpReader;
use crate::repository::{CreateSessionParams, SessionRecord, SessionRepository};
use crate::services::SessionAnalytics;

#[derive(Debug)]
pub struct CreateAnalyticsSessionInput<'a> {
    pub session_id: &'a SessionId,
    pub user_id: Option<&'a UserId>,
    pub analytics: &'a SessionAnalytics,
    pub session_source: SessionSource,
    pub is_bot: bool,
    pub is_ai_crawler: bool,
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
        db_pool: &DbPool,
        geoip_reader: Option<GeoIpReader>,
        content_routing: Option<Arc<dyn ContentRouting>>,
    ) -> Result<Self> {
        Ok(Self {
            geoip_reader,
            content_routing,
            session_repo: SessionRepository::new(db_pool)?,
        })
    }

    pub fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> SessionAnalytics {
        let mut builder = SessionAnalytics::builder(headers);
        if let Some(uri) = signals.uri {
            builder = builder.with_uri(uri);
        }
        if let Some(reader) = self.geoip_reader.as_ref() {
            builder = builder.with_geoip(reader);
        }
        if let Some(content_routing) = self.content_routing.as_deref() {
            builder = builder.with_content_routing(content_routing);
        }
        if let Some(caller_ip) = signals.caller_ip {
            builder = builder.with_caller_ip(caller_ip);
        }
        builder.build()
    }

    pub fn is_bot(analytics: &SessionAnalytics) -> bool {
        analytics.should_skip_tracking()
    }

    pub fn compute_fingerprint(analytics: &SessionAnalytics) -> String {
        analytics.fingerprint_hash.clone().unwrap_or_else(|| {
            use xxhash_rust::xxh64::xxh64;

            let data = format!(
                "{}|{}",
                analytics.user_agent.as_deref().unwrap_or(""),
                analytics.preferred_locale.as_deref().unwrap_or("")
            );

            format!("fp_{:016x}", xxh64(data.as_bytes(), 0))
        })
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
            utm_content: input.analytics.utm_content.as_deref(),
            utm_term: input.analytics.utm_term.as_deref(),
            utm_campaign: input.analytics.utm_campaign.as_deref(),
            is_bot: input.is_bot,
            is_ai_crawler: input.is_ai_crawler,
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
