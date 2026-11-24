//! The analytics member's composed provides: the open request-analytics
//! write (`DynRequestAnalytics`) the API's request-analytics middleware
//! records a served request through. It appends one `analytics_events` row
//! and reads nothing back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_logging::{AnalyticsEvent, AnalyticsRepository};
use systemprompt_traits::{
    DynRequestAnalytics, RequestAnalytics, RequestAnalyticsError, RequestAnalyticsEvent,
};

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
