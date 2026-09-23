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
mod requests;
mod session;
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
pub use requests::RequestAnalyticsRepository;
pub use session::{
    CreateSessionParams, SessionBehavioralData, SessionMigrationResult, SessionRecord,
    SessionRepository,
};
pub use tools::ToolAnalyticsRepository;
pub use tools::list_queries::ToolListParams;
pub use traffic::{NavigationQuery, PageQuery, TrafficAnalyticsRepository};

use crate::error::Result;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct AnalyticsRepositories {
    pub sessions: SessionRepository,
    pub costs: CostAnalyticsRepository,
    pub engagement: EngagementRepository,
    pub events: AnalyticsEventsRepository,
}

impl AnalyticsRepositories {
    pub fn new(
        db: &DbPool,
        sessions: systemprompt_traits::DynSessionStore,
        event_sink: systemprompt_traits::DynAnalyticsEventStore,
        content: systemprompt_traits::DynContentCatalogStats,
    ) -> Result<Self> {
        Ok(Self {
            sessions: SessionRepository::new(
                db,
                sessions,
                std::sync::Arc::clone(&event_sink),
                content,
            )?,
            costs: CostAnalyticsRepository::new(db)?,
            engagement: EngagementRepository::new(db)?,
            events: AnalyticsEventsRepository::new(event_sink),
        })
    }
}
