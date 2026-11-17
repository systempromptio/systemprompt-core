//! Job dispatch and bookkeeping — runs a single admitted job inside the
//! admitted job-run scopes and a panic-isolating wrapper, and records the run
//! on its `scheduled_jobs` row.
//!
//! A tick first reads the job's row: a row an operator set to
//! `enabled = false` (`infra jobs disable`) skips the tick, and so does a row
//! that cannot be read. A job with no row yet runs.
//!
//! A tick writes nothing until the job returns. A run is then recorded with
//! one UPDATE — `last_run`, `last_status`, `last_error`, `last_message`,
//! `last_instance_id` and `run_count` together — only when it failed or did
//! something: a successful run that reports itself idle
//! ([`JobResult::is_idle`]) leaves the row untouched and logs at TRACE. So
//! `last_run` is the last time the job did work or failed, not the last time
//! it was polled, and `run_count` counts those recorded runs. The tick
//! de-duplication in [`super::claim`] reads the same `last_run`; a second
//! replica may therefore repeat an idle tick, which by definition had
//! nothing to do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::AdmittedSchedule;
use systemprompt_identifiers::{Actor, InstanceId, JobName};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobResult};
use tracing::{debug, error, trace, warn};

use super::claim::{Claim, acquire_cluster_claim, acquire_node_claim};
use super::{RunningJobs, make_job_context};
use crate::error::{SchedulerError, SchedulerResult};
use crate::jobs::InventoryRefreshMemo;
use crate::models::{JobRunRecord, JobStatus};
use crate::repository::SchedulerRepository;
use crate::services::job_metrics;

pub(super) use super::claim::{ClaimPolicy, claim_policy};

pub(super) struct JobDispatch {
    pub(super) job: Arc<dyn JobTrait>,
    pub(super) job_name: JobName,
    pub(super) actor: Actor,
    pub(super) repository: SchedulerRepository,
    pub(super) app_context: Arc<AppContext>,
    pub(super) running_jobs: RunningJobs,
    pub(super) claim_policy: ClaimPolicy,
    pub(super) enforce: bool,
    pub(super) parameters: HashMap<String, String>,
    pub(super) inventory_memo: Arc<InventoryRefreshMemo>,
    pub(super) schedule: Arc<AdmittedSchedule>,
}

pub(super) async fn execute_job(dispatch: JobDispatch) {
    let JobDispatch {
        job,
        job_name,
        actor,
        repository,
        app_context,
        running_jobs,
        claim_policy,
        enforce,
        parameters,
        inventory_memo,
        schedule,
    } = dispatch;
    let instance_id = app_context.config().instance_id.clone();

    {
        let mut guard = running_jobs.lock().await;
        if guard.contains(&job_name) {
            warn!(job_name = %job_name, "Job already running, skipping this execution");
            return;
        }
        guard.insert(job_name.clone());
    }

    if !enabled_by_operator(&job_name, &repository).await {
        running_jobs.lock().await.remove(&job_name);
        return;
    }

    let claim = match claim_policy {
        ClaimPolicy::None => Claim::Free,
        ClaimPolicy::Cluster => acquire_cluster_claim(&job_name, &repository).await,
        ClaimPolicy::Node { instance_id } => {
            acquire_node_claim(&job_name, &instance_id, &repository).await
        },
    };
    let claim = match claim {
        Claim::Skip => {
            running_jobs.lock().await.remove(&job_name);
            return;
        },
        Claim::Held(guard) => Some(guard),
        Claim::Free => None,
    };

    trace!(job_name = %job_name, "Starting job");

    let budget = job.db_budget();
    let ctx = make_job_context(actor, app_context, budget, &inventory_memo)
        .with_enforce(enforce)
        .with_parameters(parameters);
    let started = std::time::Instant::now();
    let result = execute_in_scope(job.as_ref(), &job_name, &ctx, &schedule).await;
    job_metrics::record_job_run(
        &job_name,
        job_metrics::run_status(&result),
        started.elapsed(),
    );
    handle_job_result(&job_name, result, &repository, &instance_id).await;

    if let Some(claim) = claim {
        claim.release().await;
    }

    {
        let mut guard = running_jobs.lock().await;
        guard.remove(&job_name);
    }
}

async fn enabled_by_operator(job_name: &JobName, repository: &SchedulerRepository) -> bool {
    match repository.find_job(job_name).await {
        Ok(Some(row)) if !row.enabled => {
            debug!(job_name = %job_name, "job disabled on its scheduled_jobs row, skipping this tick");
            false
        },
        Ok(_) => true,
        Err(e) => {
            error!(job_name = %job_name, error = %e, "Failed to read job row; skipping this tick");
            false
        },
    }
}

async fn execute_in_scope(
    job: &dyn JobTrait,
    job_name: &JobName,
    ctx: &systemprompt_traits::JobContext,
    schedule: &AdmittedSchedule,
) -> SchedulerResult<JobResult> {
    use futures::FutureExt;
    use std::panic::AssertUnwindSafe;

    match AssertUnwindSafe(schedule.run_in_scopes(job.name(), job.execute(ctx)))
        .catch_unwind()
        .await
    {
        Ok(result) => result.map_err(|e| SchedulerError::job_execution_failed(job_name.clone(), e)),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_owned());
            error!(job_name = %job_name, panic = %msg, "Job panicked");
            Err(SchedulerError::panic(msg))
        },
    }
}

async fn handle_job_result(
    job_name: &JobName,
    result: SchedulerResult<JobResult>,
    repository: &SchedulerRepository,
    instance_id: &InstanceId,
) {
    match result {
        Ok(job_result) if job_result.is_idle() => {
            trace!(job_name = %job_name, duration_ms = job_result.duration_ms, "Job idle");
        },
        Ok(job_result) if job_result.success => {
            record_success(job_name, &job_result, repository, instance_id).await;
        },
        Ok(job_result) => {
            record_failure(
                job_name,
                job_result.message.as_deref(),
                repository,
                instance_id,
            )
            .await;
            error!(job_name = %job_name, message = ?job_result.message, "Job failed");
        },
        Err(e) => {
            let error_msg = e.to_string();
            error!(error = %error_msg, "Job failed with error");
            record_failure(job_name, Some(&error_msg), repository, instance_id).await;
        },
    }
}

async fn record_success(
    job_name: &JobName,
    job_result: &JobResult,
    repository: &SchedulerRepository,
    instance_id: &InstanceId,
) {
    if let Err(e) = repository
        .update_job_execution(
            job_name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                message: job_result.message.as_deref(),
                next_run: None,
                instance_id,
            },
        )
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
    job_name: &JobName,
    message: Option<&str>,
    repository: &SchedulerRepository,
    instance_id: &InstanceId,
) {
    if let Err(e) = repository
        .update_job_execution(
            job_name,
            JobRunRecord {
                status: JobStatus::Failed,
                error: message,
                message: None,
                next_run: None,
                instance_id,
            },
        )
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to update failed job status");
    }
}
