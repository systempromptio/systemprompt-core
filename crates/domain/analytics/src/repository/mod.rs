//! Repository layer.
//!
//! Typed `*Repository` structs that wrap `DbPool` and expose compile-time-
//! verified `sqlx::query!` calls for every analytics aggregation, mutation,
//! and lookup. Public re-exports below form the only supported entry points;
//! internal submodules are private to the crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cli_sessions;
mod content_audience;
mod engagement;
mod events;
mod fingerprint;
mod overview;
mod retention;
mod session_signals;
mod traffic;

pub use cli_sessions::CliSessionAnalyticsRepository;
pub use content_audience::{ContentAudienceRepository, ContentViewsEntry, ContentViewsRow};
pub use engagement::EngagementRepository;
pub use events::AnalyticsEventsRepository;
pub use fingerprint::{
    ABUSE_THRESHOLD_FOR_BAN, FingerprintRepository, HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM,
    MAX_SESSIONS_PER_FINGERPRINT, SUSTAINED_VELOCITY_MINUTES,
};
pub use overview::OverviewAnalyticsRepository;
pub use retention::AnalyticsRetention;
pub use session_signals::SessionSignalsRepository;
pub use traffic::{NavigationQuery, PageQuery, TrafficAnalyticsRepository};

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_logging::AnalyticsRepository;
use systemprompt_traits::DynSessionTelemetry;

#[derive(Clone)]
pub struct AnalyticsRepositories {
    pub session_store: DynSessionTelemetry,
    pub session_signals: SessionSignalsRepository,
    pub engagement: EngagementRepository,
    pub events: AnalyticsEventsRepository,
    pub event_log: Arc<AnalyticsRepository>,
}

impl std::fmt::Debug for AnalyticsRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsRepositories")
            .field("session_signals", &self.session_signals)
            .field("engagement", &self.engagement)
            .field("events", &self.events)
            .field("event_log", &self.event_log)
            .finish_non_exhaustive()
    }
}

impl AnalyticsRepositories {
    pub fn new(
        db: &DbPool,
        sessions: DynSessionTelemetry,
        event_log: Arc<AnalyticsRepository>,
    ) -> Self {
        let event_sink: systemprompt_traits::DynAnalyticsEventStore =
            Arc::<AnalyticsRepository>::clone(&event_log);
        Self {
            session_signals: SessionSignalsRepository::new(
                db,
                Arc::clone(&sessions),
                Arc::clone(&event_sink),
            ),
            session_store: sessions,
            engagement: EngagementRepository::new(db),
            events: AnalyticsEventsRepository::new(event_sink),
            event_log,
        }
    }
}
