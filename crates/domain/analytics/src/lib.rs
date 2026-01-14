#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::use_self)]

pub mod error;
pub mod models;
pub mod repository;
pub mod services;

pub use error::{AnalyticsError, Result as AnalyticsResult};

pub use models::{
    ActivityTrend, AnalyticsEvent, AnalyticsSession, BotTrafficStats, BrowserBreakdown,
    ContentStat, ConversationByAgent, ConversationSummary, ConversationTrend, CostOverview,
    CreateEngagementEventInput, DeviceBreakdown, EngagementEvent, EngagementEventData,
    EngagementOptionalMetrics, ErrorSummary, FingerprintAnalysisResult, FingerprintReputation,
    FlagReason, GeographicBreakdown, PlatformOverview, RecentConversation, TopAgent, TopTool,
    TopUser, TrafficSource, TrafficSummary, UserMetricsWithTrends,
};
pub use repository::{
    AnalyticsQueryRepository, CoreStatsRepository, CreateSessionParams, EngagementRepository,
    FingerprintRepository, ProviderUsage, SessionBehavioralData, SessionEngagementSummary,
    SessionMigrationResult, SessionRecord, SessionRepository, ABUSE_THRESHOLD_FOR_BAN,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, MAX_SESSIONS_PER_FINGERPRINT,
    SUSTAINED_VELOCITY_MINUTES,
};
pub use services::{
    detection, AnalyticsService, AnomalyCheckResult, AnomalyDetectionService, AnomalyEvent,
    AnomalyLevel, AnomalyThresholdConfig, BehavioralAnalysisInput, BehavioralAnalysisResult,
    BehavioralBotDetector, BehavioralSignal, CreateAnalyticsSessionInput, EscalationCriteria,
    SessionAnalytics, SessionCleanupService, SignalType, ThrottleLevel, ThrottleService,
    BEHAVIORAL_BOT_THRESHOLD,
};

pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;
