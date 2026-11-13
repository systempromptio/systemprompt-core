//! Which jobs a manual run may name: the jobs of enabled extensions, from the
//! extension manifest and the owner-attributed inventory. A name resolves to
//! a runnable job, to a linked job whose owning extension the profile
//! disables, or to nothing; the two refusals report differently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::JobName;
use systemprompt_traits::{Job, registered_jobs};

use super::{JobExecutionService, JobSelection};
use crate::error::{SchedulerError, SchedulerResult};
use crate::services::EnabledExtensions;

pub(super) enum JobLookup {
    Runnable(RunnableJob),
    OwnerDisabled(&'static str),
    NotLinked,
}

pub(super) enum RunnableJob {
    Manifest(Arc<dyn Job>),
    Inventory(&'static dyn Job),
}

impl RunnableJob {
    pub(super) fn as_job(&self) -> &dyn Job {
        match self {
            Self::Manifest(job) => job.as_ref(),
            Self::Inventory(job) => *job,
        }
    }
}

impl JobExecutionService {
    pub fn resolve_job_names(&self, selection: &JobSelection) -> SchedulerResult<Vec<JobName>> {
        match selection {
            JobSelection::All => {
                let enabled = self.enabled()?;
                let mut names: Vec<JobName> = self
                    .enabled_manifest_jobs()?
                    .into_iter()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect();
                for job in enabled.jobs() {
                    if job.enabled() && !names.iter().any(|name| name == job.name()) {
                        names.push(JobName::new(job.name()));
                    }
                }
                names.sort_unstable();
                Ok(names)
            },
            JobSelection::Tag(tag) => {
                let mut jobs: Vec<&dyn Job> = Vec::new();
                let manifest = self.enabled_manifest_jobs()?;
                jobs.extend(manifest.iter().map(AsRef::as_ref));
                let enabled = self.enabled()?;
                for job in enabled.jobs() {
                    if !jobs.iter().any(|seen| seen.name() == job.name()) {
                        jobs.push(job);
                    }
                }
                jobs.retain(|job| job.tags().contains(&tag.as_str()));
                if jobs.is_empty() {
                    return Err(SchedulerError::NoJobsWithTag { tag: tag.clone() });
                }
                Ok(jobs
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

    fn enabled_manifest_jobs(&self) -> SchedulerResult<Vec<Arc<dyn Job>>> {
        Ok(self
            .ctx
            .extension_registry()
            .enabled_extensions(&self.ctx.profile().extensions.disabled)?
            .iter()
            .flat_map(|ext| ext.jobs())
            .collect())
    }

    pub(super) fn find_runnable(&self, job_name: &JobName) -> SchedulerResult<JobLookup> {
        if let Some(job) = self
            .enabled_manifest_jobs()?
            .into_iter()
            .find(|job| job.name() == job_name.as_str())
        {
            return Ok(JobLookup::Runnable(RunnableJob::Manifest(job)));
        }
        if let Some(job) = self.enabled()?.job(job_name.as_str()) {
            return Ok(JobLookup::Runnable(RunnableJob::Inventory(job)));
        }
        Ok(registered_jobs()
            .find(|registration| registration.job.name() == job_name.as_str())
            .map_or(JobLookup::NotLinked, |registration| {
                JobLookup::OwnerDisabled(registration.owner)
            }))
    }
}
