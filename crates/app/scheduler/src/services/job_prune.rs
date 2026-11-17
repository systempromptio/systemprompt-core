//! Explicit retirement of `scheduled_jobs` rows.
//!
//! A node's boot never deletes a row, so a job no admitted member contributes
//! any longer keeps its row until an operator removes it. [`JobPruneService`]
//! computes those rows against the jobs of the admitted composition
//! ([`AdmittedSchedule`]) and deletes exactly the rows it listed. A disabled
//! member is not admitted, so its jobs' rows are listed as prunable; an
//! operator pruning from a profile that disables a member another node runs
//! must say so at the confirmation.
//!
//! An empty job set lists and deletes nothing: a composition that admitted no
//! jobs is never read as "every job is retired".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_extension::AdmittedSchedule;
use systemprompt_identifiers::JobName;

use crate::error::SchedulerResult;
use crate::models::ScheduledJob;
use crate::repository::JobRepository;

/// Lists and deletes the `scheduled_jobs` rows no admitted job names.
#[derive(Debug, Clone)]
pub struct JobPruneService {
    repository: JobRepository,
    registered: HashSet<JobName>,
}

impl JobPruneService {
    #[must_use]
    pub fn new(repository: JobRepository, schedule: &AdmittedSchedule) -> Self {
        let registered = schedule.job_names().map(JobName::new).collect();
        Self {
            repository,
            registered,
        }
    }

    pub async fn unregistered(&self) -> SchedulerResult<Vec<ScheduledJob>> {
        if self.registered.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .repository
            .list_jobs()
            .await?
            .into_iter()
            .filter(|row| !self.registered.contains(&row.job_name))
            .collect())
    }

    pub async fn delete(&self, names: &[JobName]) -> SchedulerResult<u64> {
        let registered: Vec<JobName> = self.registered.iter().cloned().collect();
        self.repository
            .delete_unregistered_jobs(names, &registered)
            .await
    }
}
