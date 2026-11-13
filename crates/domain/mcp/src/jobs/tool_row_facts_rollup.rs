//! Every-minute job draining the `tool_row_facts` projection: executions an
//! upgrade queued before their input summary, builtin flag and artifact kind
//! were stamped. New rows are stamped by the mcp schema's statement triggers;
//! this only works off the backlog, `BATCH` keys at a time until the queue is
//! empty or the tick has spent `BUDGET`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

const PROJECTION: &str = "tool_row_facts";
const PROJECTOR: &str = "refresh_tool_row_facts";
const BATCH: i32 = 500;
const BUDGET: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy)]
pub struct ToolRowFactsRollupJob;

#[async_trait]
impl Job for ToolRowFactsRollupJob {
    fn name(&self) -> &'static str {
        "tool_row_facts_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued tool executions into their stored input summary, builtin flag and artifact kind"
    }

    fn schedule(&self) -> &'static str {
        "15 * * * * *"
    }

    fn default_schedule(&self) -> Option<&'static str> {
        Some("15 * * * * *")
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
                "tool row facts backlog drained"
            );
        }
        Ok(JobResult::success()
            .with_stats(report.keys, report.requeued)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ToolRowFactsRollupJob);
