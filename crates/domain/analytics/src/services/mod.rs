//! Service layer.
//!
//! Orchestrators on top of the repository layer. Hosts the
//! [`AnalyticsService`], anomaly/behavioural detectors, request extractors,
//! and provider integrations consumed by the API and CLI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_crawler_keywords;
mod ai_provider;
mod anomaly_detection;
mod behavioral_detector;
pub mod bot_keywords;
pub mod detection;
mod extractor;
mod providers;
mod service;
mod session_cleanup;
mod user_agent;

pub use ai_provider::AnalyticsAiSessionProvider;
pub use anomaly_detection::{
    AnomalyCheckResult, AnomalyDetectionService, AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig,
};
pub use behavioral_detector::{
    BEHAVIORAL_BOT_THRESHOLD, BehavioralAnalysisInput, BehavioralAnalysisResult,
    BehavioralBotDetector, BehavioralSignal, SignalType,
};
pub use extractor::SessionAnalytics;
pub use service::{AnalyticsService, CreateAnalyticsSessionInput};
pub use session_cleanup::SessionCleanupService;
