pub mod error;
pub mod extension;
pub mod models;
pub mod repository;
pub mod services;

pub use extension::AnalyticsExtension;

pub use error::{AnalyticsError, Result as AnalyticsResult};

pub use models::{
    ActivityTrend, AnalyticsEvent, AnalyticsEventBatchResponse, AnalyticsEventCreated,
    AnalyticsEventType, AnalyticsSession, BotTrafficStats, BrowserBreakdown, ContentStat,
    ConversationByAgent, ConversationSummary, ConversationTrend, ConversionEventData, CostOverview,
    CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput, CreateEngagementEventInput,
    CreateFunnelInput, CreateFunnelStepInput, DeviceBreakdown, EngagementEvent,
    EngagementEventData, EngagementOptionalMetrics, ErrorSummary, FingerprintAnalysisResult,
    FingerprintReputation, FlagReason, Funnel, FunnelMatchType, FunnelProgress, FunnelStats,
    FunnelStep, FunnelStepStats, FunnelWithSteps, GeographicBreakdown, LinkClickEventData,
    PlatformOverview, RecentConversation, ScrollEventData, TopAgent, TopTool, TopUser,
    TrafficSource, TrafficSummary, UserMetricsWithTrends,
};
pub use repository::{
    ABUSE_THRESHOLD_FOR_BAN, AgentAnalyticsRepository, AnalyticsEventsRepository,
    AnalyticsQueryRepository, CliSessionAnalyticsRepository, ContentAnalyticsRepository,
    ConversationAnalyticsRepository, CoreStatsRepository, CostAnalyticsRepository,
    CreateSessionParams, EngagementRepository, FingerprintRepository, FunnelRepository,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, MAX_SESSIONS_PER_FINGERPRINT,
    OverviewAnalyticsRepository, ProviderUsage, RequestAnalyticsRepository,
    SUSTAINED_VELOCITY_MINUTES, SessionBehavioralData, SessionEngagementSummary,
    SessionMigrationResult, SessionRecord, SessionRepository, StoredAnalyticsEvent,
    ToolAnalyticsRepository, TrafficAnalyticsRepository,
};
pub use services::bot_keywords::matches_bot_pattern;
pub use services::{
    AnalyticsAiSessionProvider, AnalyticsService, AnomalyCheckResult, AnomalyDetectionService,
    AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig, BEHAVIORAL_BOT_THRESHOLD,
    BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralBotDetector, BehavioralSignal,
    CreateAnalyticsSessionInput, EscalationCriteria, SessionAnalytics, SessionCleanupService,
    SignalType, ThrottleLevel, ThrottleService, detection,
};

#[cfg(feature = "geolocation")]
pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;
#[cfg(not(feature = "geolocation"))]
pub type GeoIpReader = std::sync::Arc<()>;
