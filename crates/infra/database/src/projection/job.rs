//! Shared tick for the drainer jobs of SQL-refreshed projections.
//!
//! [`drain_projection`] drains one projection through its
//! `refresh_<projection>(keys TEXT[])` function within a time budget and
//! reports the keys done and requeued as job stats.
//!
//! A job that drains projections owned by several crates does not name them:
//! each owner declares its drain against the job's name with
//! [`submit_projection_drain!`](crate::submit_projection_drain), and the job
//! drains every [`registered_drains`] entry for its name whose owning
//! extension is enabled. A projection whose owner is not linked, or is
//! disabled in the profile, is not drained.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_traits::{JobContext, JobResult, ProviderError, ProviderResult};
use tracing::info;

use crate::DbPool;

/// One projection a drainer job works off, with its batch and time budget.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionDrain {
    pub projection: &'static str,
    pub projector: &'static str,
    pub batch: i32,
    pub budget: Duration,
}

/// A [`ProjectionDrain`] the extension `owner` contributes to the job named
/// `job`.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionDrainContribution {
    pub owner: &'static str,
    pub job: &'static str,
    pub drain: ProjectionDrain,
}

inventory::collect!(ProjectionDrainContribution);

pub fn registered_drains(
    job: &str,
) -> impl Iterator<Item = &'static ProjectionDrainContribution> + '_ {
    inventory::iter::<ProjectionDrainContribution>
        .into_iter()
        .filter(move |contribution| contribution.job == job)
}

#[doc(hidden)]
pub use inventory as __inventory;

#[macro_export]
macro_rules! submit_projection_drain {
    ($owner:literal, $job:literal, $drain:expr) => {
        $crate::projection::job::__inventory::submit! {
            $crate::projection::job::ProjectionDrainContribution {
                owner: $owner,
                job: $job,
                drain: $drain,
            }
        }
    };
}

pub async fn drain_projection(
    ctx: &JobContext,
    drain: &ProjectionDrain,
) -> ProviderResult<JobResult> {
    let db_pool = Arc::clone(ctx.get::<DbPool>()?);
    let report = super::drain_with_sql(
        &db_pool.write_pool(),
        drain.projection,
        drain.batch,
        drain.budget,
        drain.projector,
    )
    .await
    .map_err(|e| ProviderError::Internal(Box::new(e)))?;
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
