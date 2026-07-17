//! Process-wide record of jobs the scheduler dropped at startup.
//!
//! [`record`] is called once, after
//! [`systemprompt_scheduler::SchedulerService::start`] returns, with the jobs
//! whose explicit owner did not resolve. [`handle_health`] reads [`degraded`]
//! to report the scheduler as `degraded` so a partially disabled scheduler
//! surfaces in monitoring instead of failing silently.
//!
//! [`handle_health`]: super::health::handle_health
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

use systemprompt_scheduler::SkippedJob;

static DEGRADED_JOBS: OnceLock<Vec<SkippedJob>> = OnceLock::new();

pub fn record(jobs: Vec<SkippedJob>) {
    if DEGRADED_JOBS.set(jobs).is_err() {
        tracing::warn!("scheduler degraded-job record already set, ignoring repeat");
    }
}

pub fn degraded() -> &'static [SkippedJob] {
    DEGRADED_JOBS.get().map_or(&[], Vec::as_slice)
}
