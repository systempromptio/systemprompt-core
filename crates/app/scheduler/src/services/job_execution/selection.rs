//! Which jobs a manual run may name: the jobs the admitted composition
//! contributes. A disabled member is never admitted, so its jobs resolve to
//! nothing, exactly like a name no build contributes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::JobName;
use systemprompt_traits::Job;

use super::{JobExecutionService, JobSelection};
use crate::error::{SchedulerError, SchedulerResult};

pub(super) enum JobLookup {
    Runnable(Arc<dyn Job>),
    Unregistered,
}

impl JobExecutionService {
    pub fn resolve_job_names(&self, selection: &JobSelection) -> SchedulerResult<Vec<JobName>> {
        match selection {
            JobSelection::All => {
                let mut names: Vec<JobName> = self
                    .admitted_jobs()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect();
                names.sort_unstable();
                names.dedup();
                Ok(names)
            },
            JobSelection::Tag(tag) => {
                let tagged: Vec<&dyn Job> = self
                    .admitted_jobs()
                    .filter(|job| job.tags().contains(&tag.as_str()))
                    .collect();
                if tagged.is_empty() {
                    return Err(SchedulerError::NoJobsWithTag { tag: tag.clone() });
                }
                Ok(tagged
                    .into_iter()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect())
            },
            JobSelection::Names(names) => {
                if names.is_empty() {
                    return Err(SchedulerError::NoJobsSelected);
                }
                Ok(names.clone())
            },
        }
    }

    fn admitted_jobs(&self) -> impl Iterator<Item = &dyn Job> {
        self.schedule.jobs().iter().map(|(_, job)| job.as_ref())
    }

    pub(super) fn find_runnable(&self, job_name: &JobName) -> JobLookup {
        self.schedule
            .job(job_name.as_str())
            .map_or(JobLookup::Unregistered, |(_, job)| {
                JobLookup::Runnable(Arc::clone(job))
            })
    }
}
