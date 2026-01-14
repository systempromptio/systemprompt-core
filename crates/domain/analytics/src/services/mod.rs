mod anomaly_detection;
mod behavioral_detector;
pub mod detection;
mod extractor;
mod service;
mod session_cleanup;
mod throttle;

pub use anomaly_detection::{
    AnomalyCheckResult, AnomalyDetectionService, AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig,
};
pub use behavioral_detector::{
    BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralBotDetector, BehavioralSignal,
    SignalType, BEHAVIORAL_BOT_THRESHOLD,
};
pub use extractor::SessionAnalytics;
pub use service::{AnalyticsService, CreateAnalyticsSessionInput};
pub use session_cleanup::SessionCleanupService;
pub use throttle::{EscalationCriteria, ThrottleLevel, ThrottleService};
