//! Shared tick for the drainer jobs of SQL-refreshed projections: drains one
//! projection through its `refresh_<projection>(keys TEXT[])` function within
//! a time budget and reports the keys done and requeued as job stats.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{DbPool, projection};
use systemprompt_traits::{JobContext, JobResult, ProviderResult};
use tracing::info;

pub(super) struct ProjectionDrain {
    pub(super) projection: &'static str,
    pub(super) projector: &'static str,
    pub(super) batch: i32,
    pub(super) budget: Duration,
}

pub(super) async fn drain_projection(
    ctx: &JobContext,
    drain: &ProjectionDrain,
) -> ProviderResult<JobResult> {
    let db_pool = Arc::clone(ctx.get::<DbPool>()?);
    let report = projection::drain_with_sql(
        &db_pool.write_pool(),
        drain.projection,
        drain.batch,
        drain.budget,
        drain.projector,
    )
    .await
    .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(Box::new(e)))?;
    let duration_ms = u64::try_from(report.elapsed.as_millis()).unwrap_or(u64::MAX);
    if report.keys > 0 {
        info!(
            projection = drain.projection,
            keys = report.keys,
            remaining = report.remaining,
            requeued = report.requeued,
            duration_ms,
            "projection backlog drained"
        );
    }
    Ok(JobResult::success()
        .with_stats(report.keys, report.requeued)
        .with_duration(duration_ms))
}
