//! Explicit retirement of `scheduled_jobs` rows.
//!
//! A node's boot never deletes a row, so a job that no build registers any
//! longer keeps its row until an operator removes it. [`JobPruneService`]
//! computes those rows against every job linked into *this* binary, whether
//! or not this profile enables the owning extension, and deletes exactly the
//! rows it listed. A job whose extension is disabled here but enabled on
//! another node is therefore never listed. It is correct only from a binary
//! that links every capability the cluster runs; a row only another binary
//! links would otherwise be listed.
//!
//! An empty registration set lists and deletes nothing: an inventory that
//! failed to load is never read as "every job is retired".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::JobName;
use systemprompt_traits::registered_jobs;

use crate::error::SchedulerResult;
use crate::models::ScheduledJob;
use crate::repository::JobRepository;

/// Lists and deletes the `scheduled_jobs` rows no job linked into this binary
/// names.
#[derive(Debug, Clone)]
pub struct JobPruneService {
    repository: JobRepository,
    registered: HashSet<JobName>,
}

impl JobPruneService {
    #[must_use]
    pub fn new(repository: JobRepository) -> Self {
        let registered = registered_jobs()
            .map(|registration| JobName::new(registration.job.name()))
            .collect();
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
