//! Scheduled jobs and retention contributions of the AI domain.
//!
//! The jobs are submitted with `submit_job!` and returned from
//! [`AiExtension::jobs`](crate::AiExtension). The scheduler's kernel jobs
//! reach this crate's tables through registrations made here: the retention
//! purges and orphan sweep `database_cleanup` dispatches to, and the hourly
//! usage rollups `backfill_rollups` drains.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_request_tool_flag_rollup;
mod retention;
mod rollups;
mod thought_signature_cleanup;
pub mod usage_anomaly_scan;

pub use ai_request_tool_flag_rollup::AiRequestToolFlagRollupJob;
pub use thought_signature_cleanup::ThoughtSignatureCleanupJob;
pub use usage_anomaly_scan::UsageAnomalyScanJob;
