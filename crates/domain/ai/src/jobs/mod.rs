//! Scheduled jobs and retention contributions of the AI domain.
//!
//! [`AiExtension`](crate::AiExtension) contributes the jobs, and through
//! `contribute` the retention purges and orphan sweep `database_cleanup`
//! dispatches to and the hourly usage rollups `backfill_rollups` drains.
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

pub(crate) fn contribute(c: &mut systemprompt_extension::Contributions<'_>) {
    c.job(AiRequestToolFlagRollupJob)
        .job(ThoughtSignatureCleanupJob)
        .job(UsageAnomalyScanJob);
    retention::contribute(c);
    rollups::contribute(c);
}
