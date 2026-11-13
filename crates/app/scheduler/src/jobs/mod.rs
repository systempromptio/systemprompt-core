//! The kernel's built-in jobs, registered via
//! [`systemprompt_provider_contracts::submit_job!`]: config reconcile, the
//! licence check-in, the inventory refresh, the drainers of kernel-owned
//! projections, session housekeeping, OTLP export, and the two jobs that
//! iterate what capability crates contribute (`database_cleanup` over
//! registered retention purges, `backfill_rollups` over registered projection
//! drains). Capability jobs live in, and are registered by, their owning
//! crates.
//!
//! Each module exposes a single zero-sized type implementing
//! [`systemprompt_traits::Job`]; submission to the inventory registry happens
//! at the bottom of each module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod analytics_event_link_facts_rollup;
pub mod backfill_rollups;
mod cleanup_inactive_sessions;
mod config_reconcile;
mod database_cleanup;
mod inventory;
pub mod licence_checkin;
mod log_summary_rollup;
#[cfg(feature = "otel")]
pub mod otlp_export;
mod outbox_backlog_rollup;
mod session_bot_type_rollup;
mod trace_facts_rollup;
pub mod vertex_discovery;

pub use analytics_event_link_facts_rollup::AnalyticsEventLinkFactsRollupJob;
pub use backfill_rollups::BackfillRollupsJob;
pub use cleanup_inactive_sessions::CleanupInactiveSessionsJob;
pub use config_reconcile::ConfigReconcileJob;
pub use database_cleanup::{DatabaseCleanupJob, RetentionTarget};
pub use inventory::{InventoryRefreshJob, InventoryRefreshMemo};
pub use licence_checkin::LicenceCheckinJob;
pub use log_summary_rollup::LogSummaryRollupJob;
#[cfg(feature = "otel")]
pub use otlp_export::OtlpExportJob;
pub use outbox_backlog_rollup::OutboxBacklogRollupJob;
pub use session_bot_type_rollup::SessionBotTypeRollupJob;
pub use trace_facts_rollup::TraceFactsRollupJob;
pub use vertex_discovery::VertexDiscoveryJob;
