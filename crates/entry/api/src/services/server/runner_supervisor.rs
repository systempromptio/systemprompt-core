//! Supervision of runner services: a runner that panics or returns while the
//! server is up is started again on the same background task, after an
//! exponential backoff from one second to a sixty-second cap. A run that
//! stayed up for five minutes resets the backoff. Every restart is logged and
//! counted in `runner_restarts_total{runner}`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;

use futures_util::FutureExt;
use systemprompt_traits::{BackgroundTasks, JobContext, RunnerService};

pub const RUNNER_RESTARTS_TOTAL: &str = "runner_restarts_total";

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const HEALTHY_RUN: Duration = Duration::from_secs(300);

pub fn supervise<C>(tasks: &BackgroundTasks, service: Arc<dyn RunnerService>, job_ctx: C)
where
    C: Fn() -> JobContext + Send + 'static,
{
    let name = service.name();
    tasks.spawn_cancellable(name, move |shutdown| async move {
        let mut backoff = INITIAL_BACKOFF;
        loop {
            let started = tokio::time::Instant::now();
            let outcome = AssertUnwindSafe(service.run(job_ctx(), shutdown.clone()))
                .catch_unwind()
                .await;
            if shutdown.is_cancelled() {
                break;
            }
            let ran_for = started.elapsed();
            if ran_for >= HEALTHY_RUN {
                backoff = INITIAL_BACKOFF;
            }
            let exit = if outcome.is_err() {
                "panicked"
            } else {
                "returned"
            };
            tracing::warn!(
                runner = name,
                exit,
                ran_for_secs = ran_for.as_secs(),
                backoff_secs = backoff.as_secs(),
                "Runner service stopped; restarting after backoff"
            );
            metrics::counter!(RUNNER_RESTARTS_TOTAL, "runner" => name).increment(1);
            tokio::select! {
                () = shutdown.cancelled() => break,
                () = tokio::time::sleep(backoff) => {},
            }
            backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
        }
    });
}
