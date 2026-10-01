//! Repository layer.
//!
//! Typed `*Repository` structs that wrap `DbPool` and expose compile-time-
//! verified `sqlx::query!` calls for every analytics aggregation, mutation,
//! and lookup. Public re-exports below form the only supported entry points;
//! internal submodules are private to the crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agents;
mod cli_sessions;
mod content_analytics;
mod conversations;
mod costs;
mod engagement;
mod events;
mod fingerprint;
mod overview;
mod ownership;
mod requests;
mod session_signals;
mod tools;
mod traffic;

pub use agents::AgentAnalyticsRepository;
pub use cli_sessions::CliSessionAnalyticsRepository;
pub use content_analytics::ContentAnalyticsRepository;
pub use conversations::ConversationAnalyticsRepository;
pub use costs::CostAnalyticsRepository;
pub use engagement::EngagementRepository;
pub use events::AnalyticsEventsRepository;
pub use fingerprint::{
    ABUSE_THRESHOLD_FOR_BAN, FingerprintRepository, HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM,
    MAX_SESSIONS_PER_FINGERPRINT, SUSTAINED_VELOCITY_MINUTES,
};
pub use overview::OverviewAnalyticsRepository;
pub use ownership::AnalyticsOwnerReassignment;
pub use requests::RequestAnalyticsRepository;
pub use session_signals::SessionSignalsRepository;
pub use tools::ToolAnalyticsRepository;
pub use tools::list_queries::ToolListParams;
pub use traffic::{NavigationQuery, PageQuery, TrafficAnalyticsRepository};

use systemprompt_database::DbPool;
use systemprompt_traits::DynSessionStore;

#[derive(Clone)]
pub struct AnalyticsRepositories {
    pub session_store: DynSessionStore,
    pub session_signals: SessionSignalsRepository,
    pub costs: CostAnalyticsRepository,
    pub engagement: EngagementRepository,
    pub events: AnalyticsEventsRepository,
}

impl std::fmt::Debug for AnalyticsRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsRepositories")
            .field("session_signals", &self.session_signals)
            .field("costs", &self.costs)
            .field("engagement", &self.engagement)
            .field("events", &self.events)
            .finish_non_exhaustive()
    }
}

impl AnalyticsRepositories {
    pub fn new(
        db: &DbPool,
        sessions: DynSessionStore,
        event_sink: systemprompt_traits::DynAnalyticsEventStore,
        content: systemprompt_traits::DynContentCatalogStats,
    ) -> Self {
        Self {
            session_signals: SessionSignalsRepository::new(
                db,
                std::sync::Arc::clone(&sessions),
                std::sync::Arc::clone(&event_sink),
                content,
            ),
            session_store: sessions,
            costs: CostAnalyticsRepository::new(db),
            engagement: EngagementRepository::new(db),
            events: AnalyticsEventsRepository::new(event_sink),
        }
    }
}
