//! Which jobs a manual run may name: the owner-attributed inventory jobs of
//! enabled extensions. A name resolves to a runnable job, to a linked job
//! whose owning extension the profile disables, or to nothing; the two
//! refusals report differently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::JobName;
use systemprompt_traits::{Job, registered_jobs};

use super::{JobExecutionService, JobSelection};
use crate::error::{SchedulerError, SchedulerResult};
use crate::services::EnabledExtensions;

pub(super) enum JobLookup {
    Runnable(&'static dyn Job),
    OwnerDisabled(&'static str),
    NotLinked,
}

impl JobExecutionService {
    pub fn resolve_job_names(&self, selection: &JobSelection) -> SchedulerResult<Vec<JobName>> {
        match selection {
            JobSelection::All => {
                let enabled = self.enabled()?;
                let mut names: Vec<JobName> = enabled
                    .jobs()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect();
                names.sort_unstable();
                names.dedup();
                Ok(names)
            },
            JobSelection::Tag(tag) => {
                let enabled = self.enabled()?;
                let tagged: Vec<&dyn Job> = enabled
                    .jobs()
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

    fn enabled(&self) -> SchedulerResult<EnabledExtensions> {
        EnabledExtensions::of(&self.ctx)
    }

    pub(super) fn find_runnable(&self, job_name: &JobName) -> SchedulerResult<JobLookup> {
        if let Some(job) = self.enabled()?.job(job_name.as_str()) {
            return Ok(JobLookup::Runnable(job));
        }
        Ok(registered_jobs()
            .find(|registration| registration.job.name() == job_name.as_str())
            .map_or(JobLookup::NotLinked, |registration| {
                JobLookup::OwnerDisabled(registration.owner)
            }))
    }
}
