//! `scheduler.unlisted_jobs`: what happens to a registered job the
//! configuration never names.
//!
//! A job is unlisted when it is enabled, schedulable and configured in this
//! build but
//! appears in neither `jobs` nor `bootstrap_jobs`. Under `default` an unlisted
//! job that declares [`systemprompt_traits::Job::default_schedule`] joins the
//! schedule as a synthesised entry owned by the system admin; under `error`
//! any unlisted job stops the scheduler from starting; every unlisted job left
//! off the schedule is logged at `WARN`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_identifiers::JobName;
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::Job as JobTrait;
use tracing::{info, warn};

use super::{SchedulerService, deployment_dependencies};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobConfig, SchedulerConfig};

#[must_use]
pub fn unlisted_job_names(config: &SchedulerConfig, boot: &BootContext) -> Vec<JobName> {
    let registered: HashMap<&'static str, &'static dyn JobTrait> =
        inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .map(|&job| (job.name(), job))
            .collect();
    collect_unlisted(config, &registered, &deployment_dependencies(boot))
        .into_iter()
        .map(|job| JobName::new(job.name()))
        .collect()
}

fn collect_unlisted(
    config: &SchedulerConfig,
    registered: &HashMap<&'static str, &'static dyn JobTrait>,
    deployment: &Dependencies,
) -> Vec<&'static dyn JobTrait> {
    let named: HashSet<&str> = config
        .jobs
        .iter()
        .map(|job| job.name.as_str())
        .chain(config.bootstrap_jobs.iter().map(JobName::as_str))
        .collect();
    let mut unlisted: Vec<&'static dyn JobTrait> = registered
        .values()
        .copied()
        .filter(|job| {
            !named.contains(job.name())
                && job.enabled()
                && job.schedulable()
                && job.configured(deployment)
        })
        .collect();
    unlisted.sort_unstable_by_key(|job| job.name());
    unlisted
}

impl SchedulerService {
    pub(super) fn adopt_unlisted_jobs(
        &mut self,
        registered: &HashMap<&'static str, &'static dyn JobTrait>,
    ) -> SchedulerResult<()> {
        let unlisted = collect_unlisted(
            &self.config,
            registered,
            &deployment_dependencies(self.app_context.boot()),
        );
        if unlisted.is_empty() {
            return Ok(());
        }
        let mode = self.config.unlisted_jobs;
        if mode == UnlistedJobs::Error {
            return Err(SchedulerError::UnlistedJobs {
                names: unlisted
                    .iter()
                    .map(|job| job.name())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
        for job in unlisted {
            let default = job
                .default_schedule()
                .filter(|_| mode == UnlistedJobs::Default);
            if let Some(schedule) = default {
                info!(
                    job = job.name(),
                    schedule, "unlisted job scheduled on its declared default schedule"
                );
                self.config
                    .jobs
                    .push(JobConfig::new(JobName::new(job.name())).with_schedule(schedule));
            } else {
                warn!(
                    job = job.name(),
                    "job is available in this build but has no scheduler.jobs entry; it will \
                     never run"
                );
            }
        }
        Ok(())
    }
}
