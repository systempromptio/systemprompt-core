//! The analytics member's composed provides: the open request-analytics
//! write (`DynRequestAnalytics`) the API's request-analytics middleware
//! records a served request through (it appends one `analytics_events` row
//! and reads nothing back); the `GeoIP` database opened from the configured
//! path; and the open `AnalyticsProvider` and `FingerprintProvider` views
//! the hosts read by trait type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_logging::{AnalyticsEvent, AnalyticsRepository};
use systemprompt_manifest::Config;
use systemprompt_traits::{
    AnalyticsProvider, DynRequestAnalytics, DynSessionTelemetry, FingerprintProvider,
    RequestAnalytics, RequestAnalyticsError, RequestAnalyticsEvent,
};

use crate::{AnalyticsService, FingerprintRepository, GeoIpDatabase};

#[derive(Debug)]
struct RequestEventLog {
    log: AnalyticsRepository,
}

#[async_trait]
impl RequestAnalytics for RequestEventLog {
    async fn record(&self, event: RequestAnalyticsEvent) -> Result<(), RequestAnalyticsError> {
        let event = AnalyticsEvent {
            user_id: event.user_id,
            session_id: event.session_id,
            context_id: event.context_id,
            event_type: event.event_type,
            event_category: event.event_category,
            severity: event.severity,
            endpoint: event.endpoint,
            error_code: event.error_code,
            response_time_ms: event.response_time_ms,
            agent_id: None,
            task_id: event.task_id,
            message: event.message,
            metadata: event.metadata,
        };
        self.log
            .log_event(&event)
            .await
            .map(drop)
            .map_err(|e| RequestAnalyticsError(Box::new(e)))
    }
}

pub(crate) fn request_analytics(r: &Resolved<'_>) -> Result<DynRequestAnalytics, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(RequestEventLog {
        log: AnalyticsRepository::new(&db),
    }))
}

pub(crate) fn geoip_database(r: &Resolved<'_>) -> Result<Arc<GeoIpDatabase>, ProvideError> {
    let config = r.get::<Arc<Config>>()?;
    GeoIpDatabase::open(config.geoip_database_path.as_deref())
        .map(Arc::new)
        .map_err(|source| ProvideError::Failed(Box::new(source)))
}

pub(crate) fn analytics_provider(
    r: &Resolved<'_>,
) -> Result<Arc<dyn AnalyticsProvider>, ProvideError> {
    let service: Arc<dyn AnalyticsProvider> = r.get::<Arc<AnalyticsService>>()?;
    Ok(service)
}

pub(crate) fn fingerprint_provider(
    r: &Resolved<'_>,
) -> Result<Arc<dyn FingerprintProvider>, ProvideError> {
    let db = r.get::<DbPool>()?;
    let sessions = r.get::<DynSessionTelemetry>()?;
    let fingerprints: Arc<dyn FingerprintProvider> =
        Arc::new(FingerprintRepository::new(&db, sessions));
    Ok(fingerprints)
}
