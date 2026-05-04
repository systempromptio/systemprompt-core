//! Job dispatch and bookkeeping — runs a single inventory-registered job
//! within a panic-isolating wrapper, records its result, and updates the
//! `scheduled_jobs` row.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobResult};
use tracing::{debug, error, warn};

use super::{RunningJobs, make_job_context};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::JobStatus;
use crate::repository::SchedulerRepository;

pub(super) async fn execute_job(
    job_name: String,
    db_pool: DbPool,
    repository: SchedulerRepository,
    app_context: Arc<AppContext>,
    running_jobs: RunningJobs,
) {
    {
        let mut guard = running_jobs.lock().await;
        if guard.contains(&job_name) {
            warn!(job_name = %job_name, "Job already running, skipping this execution");
            return;
        }
        guard.insert(job_name.clone());
    }

    debug!(job_name = %job_name, "Starting job");

    if let Err(e) = repository
        .update_job_execution(&job_name, JobStatus::Running, None, None)
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to set job status to running");
    }

    if let Err(e) = repository.increment_run_count(&job_name).await {
        error!(job_name = %job_name, error = %e, "Failed to increment run count");
    }

    let result = find_and_execute_job(&job_name, db_pool, app_context).await;
    handle_job_result(&job_name, result, &repository).await;

    {
        let mut guard = running_jobs.lock().await;
        guard.remove(&job_name);
    }
}

fn find_job(job_name: &str) -> Option<&'static dyn JobTrait> {
    inventory::iter::<&'static dyn JobTrait>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied()
}

async fn find_and_execute_job(
    job_name: &str,
    db_pool: DbPool,
    app_context: Arc<AppContext>,
) -> SchedulerResult<JobResult> {
    use futures::FutureExt;
    use std::panic::AssertUnwindSafe;

    let job = find_job(job_name).ok_or_else(|| {
        error!(job_name = %job_name, "Job not found in inventory");
        SchedulerError::job_not_found(job_name)
    })?;

    let ctx = make_job_context(db_pool, app_context);

    match AssertUnwindSafe(job.execute(&ctx)).catch_unwind().await {
        Ok(result) => result.map_err(|e| {
            SchedulerError::job_execution_failed(job_name, e.to_string())
        }),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            error!(job_name = %job_name, panic = %msg, "Job panicked");
            Err(SchedulerError::panic(msg))
        },
    }
}

async fn handle_job_result(
    job_name: &str,
    result: SchedulerResult<JobResult>,
    repository: &SchedulerRepository,
) {
    match result {
        Ok(job_result) if job_result.success => {
            record_success(job_name, &job_result, repository).await;
        },
        Ok(job_result) => {
            record_failure(job_name, job_result.message.as_deref(), repository).await;
            error!(job_name = %job_name, message = ?job_result.message, "Job failed");
        },
        Err(e) => {
            let error_msg = e.to_string();
            error!(error = %error_msg, "Job failed with error");
            record_failure(job_name, Some(&error_msg), repository).await;
        },
    }
}

async fn record_success(
    job_name: &str,
    job_result: &JobResult,
    repository: &SchedulerRepository,
) {
    if let Err(e) = repository
        .update_job_execution(job_name, JobStatus::Success, None, None)
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to update job execution status");
    }

    debug!(
        job_name = %job_name,
        duration_ms = job_result.duration_ms,
        "Job completed"
    );
}

async fn record_failure(
    job_name: &str,
    message: Option<&str>,
    repository: &SchedulerRepository,
) {
    if let Err(e) = repository
        .update_job_execution(job_name, JobStatus::Failed, message, None)
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to update failed job status");
    }
}
