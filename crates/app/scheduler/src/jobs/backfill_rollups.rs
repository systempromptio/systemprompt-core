//! Every-minute job draining the three hourly usage rollups' backlogs
//! (`ai_usage_rollup_hourly`, `tool_usage_rollup_hourly`,
//! `agent_task_rollup_hourly`): the hours an upgrade queued before the
//! rollup tables existed. Each key is one UTC hour, rebuilt whole from the
//! ledger by the rollup's `refresh_*` function. New rows are rolled up by the
//! statement triggers, so once the backlog is empty a tick costs three depth
//! reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

const ROLLUPS: [(&str, &str); 3] = [
    ("ai_usage_rollup_hourly", "refresh_ai_usage_rollup_hourly"),
    (
        "tool_usage_rollup_hourly",
        "refresh_tool_usage_rollup_hourly",
    ),
    (
        "agent_task_rollup_hourly",
        "refresh_agent_task_rollup_hourly",
    ),
];
const BATCH: i32 = 24;
const BUDGET: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy)]
pub struct BackfillRollupsJob;

#[async_trait]
impl Job for BackfillRollupsJob {
    fn name(&self) -> &'static str {
        "backfill_rollups"
    }

    fn description(&self) -> &'static str {
        "Rebuilds queued hours of the hourly AI, tool and agent-task usage rollups from the ledger"
    }

    fn schedule(&self) -> &'static str {
        "45 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let db_pool = std::sync::Arc::clone(ctx.get::<DbPool>()?);
        let pool = db_pool.write_pool();
        let mut keys = 0u64;
        let mut requeued = 0u64;
        let mut elapsed = Duration::ZERO;
        for (rollup, projector) in ROLLUPS {
            let report = projection::drain_with_sql(&pool, rollup, BATCH, BUDGET, projector)
                .await
                .map_err(|e| {
                    systemprompt_provider_contracts::ProviderError::Internal(Box::new(e))
                })?;
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
