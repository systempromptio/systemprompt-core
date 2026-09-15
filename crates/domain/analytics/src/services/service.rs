//! Request signal extraction and analytics service composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use http::HeaderMap;

use systemprompt_models::ContentRouting;
use systemprompt_traits::ExtractSignals;

use crate::GeoIpReader;
use crate::repository::{AnalyticsRepositories, CostAnalyticsRepository, SessionRepository};
use crate::services::{ProfileUsageService, SessionAnalytics, SessionAnalyticsBuilder};

#[derive(Clone)]
pub struct AnalyticsService {
    geoip_reader: Option<GeoIpReader>,
    content_routing: Option<Arc<dyn ContentRouting>>,
    session_repo: SessionRepository,
    cost_repo: CostAnalyticsRepository,
}

impl std::fmt::Debug for AnalyticsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsService")
            .field("geoip_reader", &self.geoip_reader.is_some())
            .field("content_routing", &self.content_routing.is_some())
            .field("session_repo", &"SessionRepository")
            .field("cost_repo", &"CostAnalyticsRepository")
            .finish()
    }
}

impl AnalyticsService {
    pub fn new(
        geoip_reader: Option<GeoIpReader>,
        content_routing: Option<Arc<dyn ContentRouting>>,
        repositories: &AnalyticsRepositories,
    ) -> Self {
        Self {
            geoip_reader,
            content_routing,
            session_repo: repositories.sessions.clone(),
            cost_repo: repositories.costs.clone(),
        }
    }

    pub fn extract_analytics(
        &self,
        headers: &HeaderMap,
        signals: ExtractSignals<'_>,
    ) -> SessionAnalytics {
        let mut builder = SessionAnalyticsBuilder::new(headers);
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


    pub const fn cost_repo(&self) -> &CostAnalyticsRepository {
        &self.cost_repo
    }

    #[must_use]
    pub fn profile_usage(&self) -> ProfileUsageService {
        ProfileUsageService::new(self.cost_repo.clone())
    }

    pub const fn session_repo(&self) -> &SessionRepository {
        &self.session_repo
    }
}
