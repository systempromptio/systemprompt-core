//! Every-minute job draining the hourly usage rollups' backlogs: the hours an
//! upgrade queued before the rollup tables existed. The rollups are declared
//! by their owners against [`JOB_NAME`] with
//! `systemprompt_database::submit_projection_drain!` (the AI crate's
//! `ai_usage_rollup_hourly` and `ai_context_rollup_hourly`, the analytics
//! crate's `tool_usage_rollup_hourly` and `agent_task_rollup_hourly`); a
//! rollup whose owner is not linked is not drained. Each key is one UTC hour,
//! rebuilt whole from the ledger by the rollup's `refresh_*` function. New rows
//! are rolled up by the statement triggers, so once the backlog is empty a tick
//! costs one depth read per rollup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

pub const JOB_NAME: &str = "backfill_rollups";

#[derive(Debug, Clone, Copy)]
pub struct BackfillRollupsJob;

#[async_trait]
impl Job for BackfillRollupsJob {
    fn name(&self) -> &'static str {
        JOB_NAME
    }

    fn description(&self) -> &'static str {
        "Rebuilds queued hours of the hourly AI, tool and agent-task usage rollups from the ledger"
    }

    fn schedule(&self) -> &'static str {
        "45 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("45 * * * * *")
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let db_pool = std::sync::Arc::clone(ctx.get::<DbPool>()?);
        let pool = db_pool.write_pool();
        let mut keys = 0u64;
        let mut requeued = 0u64;
        let mut elapsed = Duration::ZERO;
        for drain in projection::registered_drains(JOB_NAME) {
            let rollup = drain.projection;
            let report = projection::drain_with_sql(
                &pool,
                rollup,
                drain.batch,
                drain.budget,
                drain.projector,
            )
            .await
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(Box::new(e)))?;
            if report.keys > 0 {
                info!(
                    rollup,
                    hours = report.keys,
                    remaining = report.remaining,
                    requeued = report.requeued,
                    "usage rollup backlog drained"
                );
            }
            keys = keys.saturating_add(report.keys);
            requeued = requeued.saturating_add(report.requeued);
            elapsed = elapsed.saturating_add(report.elapsed);
        }
        let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        Ok(JobResult::success()
            .with_stats(keys, requeued)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&BackfillRollupsJob);
