//! Service layer.
//!
//! Orchestrators on top of the repository layer. Hosts the
//! [`AnalyticsService`], behavioural detectors, request extractors,
//! and provider integrations consumed by the API and CLI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_crawler_keywords;
mod behavioral_detector;
pub mod bot_keywords;
pub mod detection;
pub(crate) mod extractor;
mod profile_usage;
mod providers;
mod service;
mod user_agent;

pub use behavioral_detector::{
    BEHAVIORAL_BOT_THRESHOLD, BehavioralAnalysisInput, BehavioralAnalysisResult,
    BehavioralBotDetector, BehavioralSignal, SignalType,
};
pub use extractor::{SessionAnalytics, SessionAnalyticsBuilder};
pub use profile_usage::ProfileUsageService;
pub use service::AnalyticsService;
