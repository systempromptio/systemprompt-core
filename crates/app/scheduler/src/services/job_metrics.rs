//! Prometheus counters and durations for scheduled job runs.
//!
//! Every tick that actually runs a job (the claim was won) records
//! `scheduler_job_runs_total{job,status}` and
//! `scheduler_job_duration_seconds{job}`; `status` is `success`, `idle` or
//! `error`. Job names come from the compiled-in inventory, so the label set is
//! closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::JobName;
use systemprompt_traits::JobResult;

pub const SCHEDULER_JOB_RUNS_TOTAL: &str = "scheduler_job_runs_total";
pub const SCHEDULER_JOB_DURATION_SECONDS: &str = "scheduler_job_duration_seconds";

pub const JOB_DURATION_BUCKETS: [f64; 12] = [
    0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 15.0, 60.0, 300.0, 1800.0,
];

#[must_use]
pub const fn run_status<E>(result: &Result<JobResult, E>) -> &'static str {
    match result {
        Ok(outcome) if outcome.is_idle() => "idle",
        Ok(_) => "success",
        Err(_) => "error",
    }
}

pub fn record_job_run(job: &JobName, status: &'static str, elapsed: Duration) {
    metrics::counter!(
        SCHEDULER_JOB_RUNS_TOTAL,
        "job" => job.as_str().to_owned(),
        "status" => status,
    )
    .increment(1);
    metrics::histogram!(SCHEDULER_JOB_DURATION_SECONDS, "job" => job.as_str().to_owned())
        .record(elapsed.as_secs_f64());
}
