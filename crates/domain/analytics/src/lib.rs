pub mod error;
pub mod models;
pub mod repository;
pub mod services;

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
    AgentAnalyticsRepository, AnalyticsEventsRepository, AnalyticsQueryRepository,
    CliSessionAnalyticsRepository, ContentAnalyticsRepository, ConversationAnalyticsRepository,
    CoreStatsRepository, CostAnalyticsRepository, CreateSessionParams, EngagementRepository,
    FingerprintRepository, FunnelRepository, OverviewAnalyticsRepository, ProviderUsage,
    RequestAnalyticsRepository, SessionBehavioralData, SessionEngagementSummary,
    SessionMigrationResult, SessionRecord, SessionRepository, StoredAnalyticsEvent,
    ToolAnalyticsRepository, TrafficAnalyticsRepository, ABUSE_THRESHOLD_FOR_BAN,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, MAX_SESSIONS_PER_FINGERPRINT,
    SUSTAINED_VELOCITY_MINUTES,
};
pub use services::{
    detection, AnalyticsAiSessionProvider, AnalyticsService, AnomalyCheckResult,
    AnomalyDetectionService, AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig,
    BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralBotDetector, BehavioralSignal,
    CreateAnalyticsSessionInput, EscalationCriteria, SessionAnalytics, SessionCleanupService,
    SignalType, ThrottleLevel, ThrottleService, BEHAVIORAL_BOT_THRESHOLD,
};

pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;
