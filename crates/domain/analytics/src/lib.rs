//! Analytics domain crate for systemprompt.io.
//!
//! Provides session, fingerprint, engagement, traffic and content-audience
//! analytics on top of the `systemprompt-database`
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
mod geoip;
pub mod jobs;
pub mod models;
mod provides;
pub mod repository;
pub mod resource_metrics;
pub mod services;

pub use extension::AnalyticsExtension;
pub use geoip::{GeoIpDatabase, GeoIpUnreadable};

pub use error::{AnalyticsError, Result};

pub use models::{
    AnalyticsEventBatchResponse, AnalyticsEventCreated, AnalyticsEventType, ConversionEventData,
    CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput, CreateEngagementEventInput,
    EngagementEvent, EngagementEventData, EngagementOptionalMetrics, FingerprintAnalysisResult,
    FingerprintReputation, FlagReason, LinkClickEventData, ScrollEventData,
};
pub use repository::{
    ABUSE_THRESHOLD_FOR_BAN, AnalyticsEventsRepository, AnalyticsRetention,
    CliSessionAnalyticsRepository, ContentAudienceRepository, ContentViewsEntry, ContentViewsRow,
    EngagementRepository, FingerprintRepository, HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM,
    MAX_SESSIONS_PER_FINGERPRINT, NavigationQuery, OverviewAnalyticsRepository, PageQuery,
    SUSTAINED_VELOCITY_MINUTES, SessionSignalsRepository, TrafficAnalyticsRepository,
};
pub use services::{
    AnalyticsService, BEHAVIORAL_BOT_THRESHOLD, BehavioralAnalysisInput, BehavioralAnalysisResult,
    BehavioralBotDetector, BehavioralSignal, SessionAnalytics, SessionAnalyticsBuilder,
    SessionBehaviourService, SignalType, collect_analysis_input, detection,
};

#[cfg(feature = "geolocation")]
pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;

#[cfg(not(feature = "geolocation"))]
pub type GeoIpReader = std::sync::Arc<()>;
