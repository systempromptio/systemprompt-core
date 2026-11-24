//! Per-instant job claims: who runs a job when several replicas tick together.
//!
//! A tick is a [`Trigger::Scheduled`] for the cron instant it belongs to
//! (see [`super::instants`]) or a [`Trigger::Bootstrap`] run at scheduler
//! start.
//!
//! [`ClaimPolicy::None`] is the single-process case (no distributed lock
//! configured): every tick runs and nothing is claimed.
//! [`ClaimPolicy::Cluster`] takes the job's cross-replica advisory lock, which
//! excludes a run that overlaps one still going on another replica, and then
//! claims the instant's
//! run row ([`crate::repository::JobRunRepository::claim`]); the row insert
//! is the judgement, so of two replicas ticking for one instant exactly one
//! runs, whatever their clocks read and however late the second arrives.
//! [`ClaimPolicy::Node`] claims the instant's row under its own instance, so
//! it runs once per instant on every replica and other replicas never starve
//! it. A bootstrap run is per boot, not per instant: a cluster-scoped one runs
//! under the advisory lock alone.
//!
//! The claim fails closed. A database error while taking the lock or claiming
//! the row skips the tick (the run for that instant may then be lost: at most
//! once under a database outage), never runs it unclaimed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{InstanceId, JobName};
use systemprompt_traits::{Job as JobTrait, JobScope};
use tracing::{debug, error};

use crate::models::{JobConfig, SchedulerConfig};
use crate::repository::{JobLockGuard, RunClaim, RunKey, SchedulerRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClaimPolicy {
    None,
    Cluster,
    Node { instance_id: InstanceId },
}

impl ClaimPolicy {
    pub(super) fn run_key(&self) -> Option<RunKey> {
        match self {
            Self::None => None,
            Self::Cluster => Some(RunKey::Cluster),
            Self::Node { instance_id } => Some(RunKey::Node(instance_id.clone())),
        }
    }
}

pub(crate) fn claim_policy(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trigger {
    Scheduled(DateTime<Utc>),
    Bootstrap,
}

pub(super) enum Claim {
    Run {
        lock: Option<JobLockGuard>,
        run: Option<RunClaim>,
    },
    Skip,
}

pub(super) async fn acquire(
    policy: &ClaimPolicy,
    trigger: Trigger,
    job_name: &JobName,
    claimer: &InstanceId,
    repository: &SchedulerRepository,
) -> Claim {
    let lock = match policy {
        ClaimPolicy::None => {
            return Claim::Run {
                lock: None,
                run: None,
            };
        },
        ClaimPolicy::Cluster => match repository.try_acquire_job_lock(job_name).await {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => {
                skipped(job_name, "job is running on another replica");
                return Claim::Skip;
            },
            Err(e) => {
                error!(job_name = %job_name, error = %e, "Failed to acquire distributed job lock; skipping this tick");
                return Claim::Skip;
            },
        },
        ClaimPolicy::Node { .. } => None,
    };
    let (Trigger::Scheduled(scheduled_at), Some(key)) = (trigger, policy.run_key()) else {
        return Claim::Run { lock, run: None };
    };
    match repository
        .runs()
        .claim(job_name, scheduled_at, &key, claimer)
        .await
    {
        Ok(Some(run)) => Claim::Run {
            lock,
            run: Some(run),
        },
        Ok(None) => {
            release(lock).await;
            skipped(job_name, "instant already claimed");
            Claim::Skip
        },
        Err(e) => {
            release(lock).await;
            error!(job_name = %job_name, %scheduled_at, error = %e, "Failed to claim the job's run for this instant; skipping it");
            Claim::Skip
        },
    }
}

async fn release(lock: Option<JobLockGuard>) {
    if let Some(lock) = lock {
        lock.release().await;
    }
}

fn skipped(job_name: &JobName, reason: &'static str) {
    debug!(
        monotonic_counter.scheduler_job_skipped_by_lock = 1u64,
        job_name = %job_name,
        reason,
        event = "scheduler.job.skipped_by_lock",
        "job's run for this tick is claimed elsewhere, skipping"
    );
}
