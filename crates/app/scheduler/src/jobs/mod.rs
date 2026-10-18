//! Built-in job implementations registered via
//! [`systemprompt_provider_contracts::submit_job!`].
//!
//! Each module exposes a single zero-sized type implementing
//! [`systemprompt_traits::Job`]; submission to the inventory registry happens
//! at the bottom of each module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_request_tool_flag_rollup;
mod analytics_event_link_facts_rollup;
mod backfill_rollups;
mod backfill_session_geo;
mod behavioral_analysis;
mod cleanup_empty_contexts;
mod cleanup_inactive_sessions;
mod config_reconcile;
mod database_cleanup;
mod ghost_session_cleanup;
mod inventory;
mod log_summary_rollup;
mod malicious_ip_blacklist;
mod no_js_cleanup;
pub mod otlp_export;
mod projection_drain;
mod session_bot_type_rollup;
mod thought_signature_cleanup;
mod tool_row_facts_rollup;
mod trace_facts_rollup;
pub mod usage_anomaly_scan;
mod user_context_counters_rollup;
pub mod vertex_discovery;

pub use ai_request_tool_flag_rollup::AiRequestToolFlagRollupJob;
pub use analytics_event_link_facts_rollup::AnalyticsEventLinkFactsRollupJob;
pub use backfill_rollups::BackfillRollupsJob;
pub use backfill_session_geo::BackfillSessionGeoJob;
pub use behavioral_analysis::BehavioralAnalysisJob;
pub use cleanup_empty_contexts::CleanupEmptyContextsJob;
pub use cleanup_inactive_sessions::CleanupInactiveSessionsJob;
pub use config_reconcile::ConfigReconcileJob;
pub use database_cleanup::DatabaseCleanupJob;
pub use ghost_session_cleanup::GhostSessionCleanupJob;
pub use inventory::InventoryRefreshJob;
pub use log_summary_rollup::LogSummaryRollupJob;
pub use malicious_ip_blacklist::MaliciousIpBlacklistJob;
pub use no_js_cleanup::NoJsCleanupJob;
pub use otlp_export::OtlpExportJob;
pub use session_bot_type_rollup::SessionBotTypeRollupJob;
pub use thought_signature_cleanup::ThoughtSignatureCleanupJob;
pub use tool_row_facts_rollup::ToolRowFactsRollupJob;
pub use trace_facts_rollup::TraceFactsRollupJob;
pub use usage_anomaly_scan::UsageAnomalyScanJob;
pub use user_context_counters_rollup::UserContextCountersRollupJob;
pub use vertex_discovery::VertexDiscoveryJob;
