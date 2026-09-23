//! Public DTO/row types for the analytics crate.
//!
//! Submodules are split by domain (engagement, events, fingerprint)
//! plus a `reporting` family of row structs consumed by report frontends such
//! as `systemprompt-cli`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod engagement;
mod events;
mod fingerprint;
pub mod reporting;

pub use engagement::{CreateEngagementEventInput, EngagementEvent, EngagementOptionalMetrics};
pub use events::{
    AnalyticsEventBatchResponse, AnalyticsEventCreated, AnalyticsEventType, ConversionEventData,
    CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput, EngagementEventData,
    LinkClickEventData, ScrollEventData,
};
pub use fingerprint::{FingerprintAnalysisResult, FingerprintReputation, FlagReason};
pub use reporting::*;

pub use systemprompt_traits::session_store::SessionSnapshot as AnalyticsSession;
