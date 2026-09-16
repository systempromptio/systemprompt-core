//! Built-in job implementations registered via
//! [`systemprompt_provider_contracts::submit_job!`].
//!
//! Each module exposes a single zero-sized type implementing
//! [`systemprompt_traits::Job`]; submission to the inventory registry happens
//! at the bottom of each module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod backfill_session_geo;
mod behavioral_analysis;
mod cleanup_empty_contexts;
mod cleanup_inactive_sessions;
mod database_cleanup;
mod evaluation_supervisor;
mod feedback_facts;
mod ghost_session_cleanup;
mod inventory;
mod malicious_ip_blacklist;
mod no_js_cleanup;
mod service_registry_gc;
mod thought_signature_cleanup;
pub mod vertex_discovery;

pub use backfill_session_geo::BackfillSessionGeoJob;
pub use behavioral_analysis::BehavioralAnalysisJob;
pub use cleanup_empty_contexts::CleanupEmptyContextsJob;
pub use cleanup_inactive_sessions::CleanupInactiveSessionsJob;
pub use database_cleanup::DatabaseCleanupJob;
pub use evaluation_supervisor::EvaluationSupervisorJob;
pub use feedback_facts::FeedbackFactsJob;
pub use ghost_session_cleanup::GhostSessionCleanupJob;
pub use inventory::InventoryRefreshJob;
pub use malicious_ip_blacklist::MaliciousIpBlacklistJob;
pub use no_js_cleanup::NoJsCleanupJob;
pub use service_registry_gc::ServiceRegistryGcJob;
pub use thought_signature_cleanup::ThoughtSignatureCleanupJob;
pub use vertex_discovery::VertexDiscoveryJob;

mod feedback_snapshots;
pub use feedback_snapshots::FeedbackSnapshotsJob;
