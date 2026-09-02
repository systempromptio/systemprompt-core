//! Job dispatch and bookkeeping — runs a single inventory-registered job
//! within a panic-isolating wrapper, records its result, and updates the
//! `scheduled_jobs` row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, InstanceId};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobResult};
use tracing::{debug, error, warn};

use super::claim::{Claim, acquire_cluster_claim, acquire_node_claim};
use super::{RunningJobs, make_job_context};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobRunRecord, JobStatus};
use crate::repository::SchedulerRepository;

pub(super) use super::claim::{ClaimPolicy, claim_policy};

pub(super) struct JobDispatch {
    pub(super) job_name: String,
    pub(super) actor: Actor,
    pub(super) db_pool: DbPool,
    pub(super) repository: SchedulerRepository,
    pub(super) app_context: Arc<AppContext>,
    pub(super) running_jobs: RunningJobs,
    pub(super) claim_policy: ClaimPolicy,
    pub(super) enforce: bool,
    pub(super) parameters: HashMap<String, String>,
}

pub(super) async fn execute_job(dispatch: JobDispatch) {
    let JobDispatch {
        job_name,
        actor,
        db_pool,
        repository,
        app_context,
        running_jobs,
        claim_policy,
        enforce,
        parameters,
    } = dispatch;
    let instance_id = InstanceId::new(&app_context.config().instance_id);

    {
        let mut guard = running_jobs.lock().await;
        if guard.contains(&job_name) {
            warn!(job_name = %job_name, "Job already running, skipping this execution");
            return;
        }
        guard.insert(job_name.clone());
    }

    let claim = match claim_policy {
        ClaimPolicy::None => Claim::Free,
        ClaimPolicy::Cluster => acquire_cluster_claim(&job_name, &db_pool, &repository).await,
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

    debug!(job_name = %job_name, "Starting job");

    if let Err(e) = repository
        .update_job_execution(
            &job_name,
            JobRunRecord {
                status: JobStatus::Running,
                error: None,
                next_run: None,
                instance_id: &instance_id,
            },
        )
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to set job status to running");
    }

    if let Err(e) = repository.increment_run_count(&job_name).await {
        error!(job_name = %job_name, error = %e, "Failed to increment run count");
    }

    let ctx = make_job_context(actor, db_pool, app_context)
        .with_enforce(enforce)
        .with_parameters(parameters);
    let result = find_and_execute_job(&job_name, &ctx).await;
    handle_job_result(&job_name, result, &repository, &instance_id).await;

    if let Some(claim) = claim {
        claim.release().await;
    }

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
    ctx: &systemprompt_traits::JobContext,
) -> SchedulerResult<JobResult> {
    use futures::FutureExt;
    use std::panic::AssertUnwindSafe;

    let job = find_job(job_name).ok_or_else(|| {
        error!(job_name = %job_name, "Job not found in inventory");
        SchedulerError::job_not_found(job_name)
    })?;

    match AssertUnwindSafe(job.execute(ctx)).catch_unwind().await {
        Ok(result) => {
            result.map_err(|e| SchedulerError::job_execution_failed(job_name, e.to_string()))
        },
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
    job_name: &str,
    result: SchedulerResult<JobResult>,
    repository: &SchedulerRepository,
    instance_id: &InstanceId,
) {
    match result {
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
    job_name: &str,
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
    job_name: &str,
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
                next_run: None,
                instance_id,
            },
        )
        .await
    {
        error!(job_name = %job_name, error = %e, "Failed to update failed job status");
    }
}
