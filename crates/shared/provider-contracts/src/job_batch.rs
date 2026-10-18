//! [`run_batched`]: the bounded batch loop a draining job runs.
//!
//! A job that works through a backlog (retention deletes, a projection
//! drain) runs one bounded batch at a time until a batch comes back short —
//! the backlog is drained — or the run's deadline passes, leaving the rest
//! to the next run. The deadline is checked before every batch, so a run
//! never starts a batch it has no time for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::Instant;

use crate::JobResult;

/// When a batched run stops: at `deadline`, or after a batch that processed
/// fewer than `full_batch` items. A `full_batch` of 1 stops only on an empty
/// batch.
#[derive(Debug, Clone, Copy)]
pub struct BatchPlan {
    pub deadline: Instant,
    pub full_batch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchedRun {
    pub processed: u64,
    pub batches: u64,
    pub complete: bool,
}

impl BatchedRun {
    #[must_use]
    pub fn into_job_result(self) -> JobResult {
        let result = JobResult::success().with_stats(self.processed, 0);
        if self.complete {
            result
        } else {
            result.with_message(format!(
                "{} item(s) in {} batch(es); run budget spent, more remain",
                self.processed, self.batches
            ))
        }
    }
}

pub async fn run_batched<F, Fut, E>(plan: BatchPlan, mut batch: F) -> Result<BatchedRun, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, E>>,
{
    let mut run = BatchedRun {
        processed: 0,
        batches: 0,
        complete: false,
    };
    while Instant::now() < plan.deadline {
        let processed = batch().await?;
        run.processed = run.processed.saturating_add(processed);
        run.batches += 1;
        if processed < plan.full_batch.max(1) {
            run.complete = true;
            break;
        }
    }
    Ok(run)
}
