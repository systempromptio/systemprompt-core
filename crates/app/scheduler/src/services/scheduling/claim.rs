//! Per-tick job claims: who may run a job when several replicas tick together.
//!
//! [`ClaimPolicy::None`] is the single-process case (no distributed lock
//! configured). [`ClaimPolicy::Cluster`] takes the cross-replica advisory
//! lock so exactly one replica runs the job. [`ClaimPolicy::Node`] runs on
//! every replica and only de-duplicates against this replica's own last run
//! (`scheduled_jobs.last_instance_id`), so other replicas never starve it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::{Job as JobTrait, JobScope};
use tracing::{debug, error, info};

use super::lock::{JobLockGuard, try_acquire_job_lock};
use crate::models::{JobConfig, SchedulerConfig};
use crate::repository::SchedulerRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ClaimPolicy {
    None,
    Cluster,
    Node { instance_id: InstanceId },
}

pub(super) fn claim_policy(
    config: &SchedulerConfig,
    job_config: Option<&JobConfig>,
    job: Option<&dyn JobTrait>,
    instance_id: &InstanceId,
) -> ClaimPolicy {
    if !config.distributed_lock {
        return ClaimPolicy::None;
    }
    let scope = job_config
        .and_then(|cfg| cfg.scope)
        .unwrap_or_else(|| job.map_or(JobScope::Cluster, JobTrait::scope));
    match scope {
        JobScope::Cluster => ClaimPolicy::Cluster,
        JobScope::Node => ClaimPolicy::Node {
            instance_id: instance_id.clone(),
        },
    }
}

pub(super) enum Claim {
    Held(JobLockGuard),
    Free,
    Skip,
}

const TICK_DEDUPE_WINDOW_MS: i64 = 900;

async fn ran_within_dedupe_window(
    job_name: &str,
    repository: &SchedulerRepository,
    same_instance: Option<&InstanceId>,
) -> bool {
    match repository.find_job(job_name).await {
        Ok(Some(job)) => {
            let instance_matches =
                same_instance.is_none_or(|id| job.last_instance_id.as_deref() == Some(id.as_str()));
            job.last_run.is_some_and(|last_run| {
                instance_matches
                    && chrono::Utc::now().signed_duration_since(last_run)
                        < chrono::Duration::milliseconds(TICK_DEDUPE_WINDOW_MS)
            })
        },
        Ok(None) => false,
        Err(e) => {
            error!(job_name = %job_name, error = %e, "Failed to read job row for tick de-duplication");
            false
        },
    }
}

pub(super) async fn acquire_node_claim(
    job_name: &str,
    instance_id: &InstanceId,
    repository: &SchedulerRepository,
) -> Claim {
    if ran_within_dedupe_window(job_name, repository, Some(instance_id)).await {
        debug!(job_name = %job_name, "node-scoped job already ran on this replica for this tick, skipping");
        return Claim::Skip;
    }
    Claim::Free
}

pub(super) async fn acquire_cluster_claim(
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

    if ran_within_dedupe_window(job_name, repository, None).await {
        guard.release().await;
        skipped_by_lock(job_name);
        return Claim::Skip;
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
