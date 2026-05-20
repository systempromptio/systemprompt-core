//! Job dispatch and bookkeeping — runs a single inventory-registered job
//! within a panic-isolating wrapper, records its result, and updates the
//! `scheduled_jobs` row.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobResult};
use tracing::{debug, error, info, warn};

use super::lock::{JobLockGuard, try_acquire_job_lock};
use super::{RunningJobs, make_job_context};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::JobStatus;
use crate::repository::SchedulerRepository;

pub(super) struct JobDispatch {
    pub(super) job_name: String,
    pub(super) actor: UserId,
    pub(super) db_pool: DbPool,
    pub(super) repository: SchedulerRepository,
    pub(super) app_context: Arc<AppContext>,
    pub(super) running_jobs: RunningJobs,
    pub(super) distributed_lock: bool,
}

pub(super) async fn execute_job(dispatch: JobDispatch) {
    let JobDispatch {
        job_name,
        actor,
        db_pool,
        repository,
        app_context,
        running_jobs,
        distributed_lock,
    } = dispatch;

    {
        let mut guard = running_jobs.lock().await;
        if guard.contains(&job_name) {
            warn!(job_name = %job_name, "Job already running, skipping this execution");
            return;
        }
        guard.insert(job_name.clone());
    }

    let claim = if distributed_lock {
        match acquire_claim(&job_name, &db_pool, &repository).await {
            Claim::Skip => {
                running_jobs.lock().await.remove(&job_name);
                return;
            },
            Claim::Held(guard) => Some(guard),
        }
    } else {
        None
    };

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

    let result = find_and_execute_job(&job_name, actor, db_pool, app_context).await;
    handle_job_result(&job_name, result, &repository).await;

    if let Some(claim) = claim {
        claim.release().await;
    }

    {
        let mut guard = running_jobs.lock().await;
        guard.remove(&job_name);
    }
}

enum Claim {
    Held(JobLockGuard),
    Skip,
}

async fn acquire_claim(
    job_name: &str,
    db_pool: &DbPool,
    repository: &SchedulerRepository,
) -> Claim {
    let write_pool = match db_pool.write_pool_arc() {
        Ok(pool) => pool,
        Err(e) => {
            error!(job_name = %job_name, error = %e, "Failed to resolve write pool for job lock");
            return Claim::Skip;
        },
    };

    let guard = match try_acquire_job_lock(&write_pool, job_name).await {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            skipped_by_lock(job_name);
            return Claim::Skip;
        },
        Err(e) => {
            error!(job_name = %job_name, error = %e, "Failed to acquire distributed job lock");
            return Claim::Skip;
        },
    };

    // Why: cron's finest granularity is 1s; a peer-completed tick within 900ms
    // means this tick is already done — skip rather than re-run.
    match repository.find_job(job_name).await {
        Ok(Some(job)) => {
            if let Some(last_run) = job.last_run {
                let since = chrono::Utc::now().signed_duration_since(last_run);
                if since < chrono::Duration::milliseconds(900) {
                    guard.release().await;
                    skipped_by_lock(job_name);
                    return Claim::Skip;
                }
            }
        },
        Ok(None) => {},
        Err(e) => {
            error!(job_name = %job_name, error = %e, "Failed to read job row for tick de-duplication");
        },
    }

    Claim::Held(guard)
}

fn skipped_by_lock(job_name: &str) {
    debug!(job_name = %job_name, "job already claimed for this tick by another replica, skipping");
    info!(
        monotonic_counter.scheduler_job_skipped_by_lock = 1u64,
        job_name = %job_name,
        event = "scheduler.job.skipped_by_lock",
    );
}

fn find_job(job_name: &str) -> Option<&'static dyn JobTrait> {
    inventory::iter::<&'static dyn JobTrait>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied()
}

async fn find_and_execute_job(
    job_name: &str,
    actor: UserId,
    db_pool: DbPool,
    app_context: Arc<AppContext>,
) -> SchedulerResult<JobResult> {
    use futures::FutureExt;
    use std::panic::AssertUnwindSafe;

    let job = find_job(job_name).ok_or_else(|| {
        error!(job_name = %job_name, "Job not found in inventory");
        SchedulerError::job_not_found(job_name)
    })?;

    let ctx = make_job_context(actor, db_pool, app_context);

    match AssertUnwindSafe(job.execute(&ctx)).catch_unwind().await {
        Ok(result) => {
            result.map_err(|e| SchedulerError::job_execution_failed(job_name, e.to_string()))
        },
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

async fn record_success(job_name: &str, job_result: &JobResult, repository: &SchedulerRepository) {
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

async fn record_failure(job_name: &str, message: Option<&str>, repository: &SchedulerRepository) {
    if let Err(e) = repository
        .update_job_execution(job_name, JobStatus::Failed, message, None)
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to update failed job status");
    }
}
