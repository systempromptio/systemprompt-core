//! Every-minute job draining the `ai_request_tool_flag` projection: requests an
//! upgrade queued before their offered-tools flag was stamped. New rows are
//! stamped by the gateway admission and the offered-tools upsert; this only
//! works off the backlog, `BATCH` keys at a time until the queue is empty or
//! the tick has spent `BUDGET`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

const PROJECTION: &str = "ai_request_tool_flag";
const PROJECTOR: &str = "refresh_ai_request_tool_flag";
const BATCH: i32 = 500;
const BUDGET: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy)]
pub struct AiRequestToolFlagRollupJob;

#[async_trait]
impl Job for AiRequestToolFlagRollupJob {
    fn name(&self) -> &'static str {
        "ai_request_tool_flag_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued AI requests into their stored offered-tools flag"
    }

    fn schedule(&self) -> &'static str {
        "55 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let db_pool = std::sync::Arc::clone(ctx.get::<DbPool>()?);
        let report =
            projection::drain_with_sql(&db_pool.write_pool(), PROJECTION, BATCH, BUDGET, PROJECTOR)
                .await
                .map_err(|e| {
                    systemprompt_provider_contracts::ProviderError::Internal(Box::new(e))
                })?;
        let duration_ms = u64::try_from(report.elapsed.as_millis()).unwrap_or(u64::MAX);
        if report.keys > 0 {
            info!(
                keys = report.keys,
                remaining = report.remaining,
                requeued = report.requeued,
                duration_ms,
                "offered-tools flag backlog drained"
            );
        }
        Ok(JobResult::success()
            .with_stats(report.keys, report.requeued)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&AiRequestToolFlagRollupJob);
