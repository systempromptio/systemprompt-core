//! Analytics domain crate for systemprompt.io.
//!
//! Provides session, fingerprint, funnel, engagement, conversation, content,
//! tool, agent, and cost analytics on top of the `systemprompt-database`
//! abstraction. Public surface is a typed [`AnalyticsError`] boundary plus a
//! family of repositories and services consumed by `systemprompt-api`,
//! `systemprompt-cli`, and `systemprompt-scheduler`.
//!
//! # Feature flags
//!
//! | Feature       | Description                                                                  |
//! |---------------|------------------------------------------------------------------------------|
//! | _(default)_   | Core analytics — repositories, services, events, no geolocation enrichment.  |
//! | `geolocation` | Enables `MaxMind` `GeoIP` enrichment via `maxminddb` for [`GeoIpReader`].        |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
pub mod extension;
pub mod models;
pub mod repository;
pub mod services;

pub use extension::AnalyticsExtension;

pub use error::{AnalyticsError, Result, Result as AnalyticsResult};

pub use models::{
    ActivityTrend, AnalyticsEvent, AnalyticsEventBatchResponse, AnalyticsEventCreated,
    AnalyticsEventType, AnalyticsSession, BotTrafficStats, BrowserBreakdown, ContentStat,
    ContextGroupRow, ContextSummaryRow, ConversationByAgent, ConversationSummary,
    ConversationTrend, ConversionEventData, CostOverview, CreateAnalyticsEventBatchInput,
    CreateAnalyticsEventInput, CreateEngagementEventInput, CreateFunnelInput,
    CreateFunnelStepInput, DeviceBreakdown, EngagementEvent, EngagementEventData,
    EngagementOptionalMetrics, ErrorSummary, FingerprintAnalysisResult, FingerprintReputation,
    FlagReason, Funnel, FunnelMatchType, FunnelProgress, FunnelStats, FunnelStep, FunnelStepStats,
    FunnelWithSteps, GeographicBreakdown, LinkClickEventData, PlatformOverview, RecentContextRow,
    RecentConversation, ScrollEventData, TopAgent, TopTool, TopUser, TrafficSource, TrafficSummary,
    UserMetricsWithTrends,
};
pub use repository::{
    ABUSE_THRESHOLD_FOR_BAN, AgentAnalyticsRepository, AnalyticsEventsRepository,
    AnalyticsQueryRepository, CliSessionAnalyticsRepository, ContentAnalyticsRepository,
    ConversationAnalyticsRepository, CoreStatsRepository, CostAnalyticsRepository,
    CreateSessionParams, EngagementRepository, FingerprintRepository, FunnelRepository,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, MAX_SESSIONS_PER_FINGERPRINT, NavigationQuery,
    OverviewAnalyticsRepository, PageQuery, ProviderUsage, RequestAnalyticsRepository,
    SUSTAINED_VELOCITY_MINUTES, SessionBehavioralData, SessionEngagementSummary,
    SessionMigrationResult, SessionRecord, SessionRepository, StoredAnalyticsEvent,
    ToolAnalyticsRepository, ToolListParams, TrafficAnalyticsRepository,
};
pub use services::bot_keywords::matches_bot_pattern;
pub use services::{
    AnalyticsAiSessionProvider, AnalyticsService, AnomalyCheckResult, AnomalyDetectionService,
    AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig, BEHAVIORAL_BOT_THRESHOLD,
    BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralBotDetector, BehavioralSignal,
    SessionAnalytics, SessionAnalyticsBuilder, SessionCleanupService, SignalType, detection,
};

#[cfg(feature = "geolocation")]
pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;

#[cfg(not(feature = "geolocation"))]
pub type GeoIpReader = std::sync::Arc<()>;
