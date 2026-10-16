//! Distributed-lock claim for a manual job run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::JobName;

use super::report::failed_report;
use super::{JobExecutionService, JobRunReport, RunnableJob};
use crate::error::SchedulerResult;
use crate::repository::JobLockGuard;
use crate::services::scheduling::{ClaimPolicy, claim_policy};

pub(super) enum Claimed {
    Held(JobLockGuard),
    Free,
    Contended,
}

impl JobExecutionService {
    pub(super) async fn claim(
        &self,
        job_name: &JobName,
        runnable: &RunnableJob,
    ) -> SchedulerResult<Claimed> {
        let job_config = self
            .scheduler_config
            .jobs
            .iter()
            .find(|job| &job.name == job_name);
        let policy = claim_policy(
            &self.scheduler_config,
            job_config,
            Some(runnable.as_job()),
            &self.ctx.config().instance_id,
        );
        if policy != ClaimPolicy::Cluster {
            return Ok(Claimed::Free);
        }
        Ok(self
            .scheduler_repo
            .acquire_job_lock_within(job_name, self.lock_wait)
            .await?
            .map_or(Claimed::Contended, Claimed::Held))
    }

    pub(super) fn contended_report(&self, job_name: &JobName, start: Instant) -> JobRunReport {
        tracing::warn!(
            job = %job_name,
            waited_ms = u64::try_from(self.lock_wait.as_millis()).unwrap_or(u64::MAX),
            "manual run skipped: another replica holds the job's distributed lock"
        );
        JobRunReport {
            lock_contended: true,
            ..failed_report(
                job_name,
                start,
                Some(format!(
                    "Job '{job_name}' is running on another replica (distributed lock held; \
                     waited {}s)",
                    self.lock_wait.as_secs()
                )),
            )
        }
    }
}
