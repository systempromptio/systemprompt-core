//! Repository layer.
//!
//! Typed `*Repository` structs that wrap `DbPool` and expose compile-time-
//! verified `sqlx::query!` calls for every analytics aggregation, mutation,
//! and lookup. Public re-exports below form the only supported entry points;
//! internal submodules are private to the crate.

mod agents;
mod cli_sessions;
mod content_analytics;
mod conversations;
mod core_stats;
mod costs;
mod engagement;
mod events;
mod fingerprint;
mod funnel;
mod overview;
mod queries;
mod requests;
mod session;
mod tools;
mod traffic;

pub use agents::AgentAnalyticsRepository;
pub use cli_sessions::CliSessionAnalyticsRepository;
pub use content_analytics::ContentAnalyticsRepository;
pub use conversations::ConversationAnalyticsRepository;
pub use core_stats::CoreStatsRepository;
pub use costs::CostAnalyticsRepository;
pub use engagement::{EngagementRepository, SessionEngagementSummary};
pub use events::{AnalyticsEventsRepository, StoredAnalyticsEvent};
pub use fingerprint::{
    ABUSE_THRESHOLD_FOR_BAN, FingerprintRepository, HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM,
    MAX_SESSIONS_PER_FINGERPRINT, SUSTAINED_VELOCITY_MINUTES,
};
pub use funnel::FunnelRepository;
pub use overview::OverviewAnalyticsRepository;
pub use queries::{AnalyticsQueryRepository, ProviderUsage};
pub use requests::RequestAnalyticsRepository;
pub use session::{
    CreateSessionParams, SessionBehavioralData, SessionMigrationResult, SessionRecord,
    SessionRepository,
};
pub use tools::ToolAnalyticsRepository;
pub use tools::list_queries::ToolListParams;
pub use traffic::TrafficAnalyticsRepository;
