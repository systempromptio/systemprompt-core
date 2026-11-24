//! Job names the configuration names but no admitted member registers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::SchedulerConfig;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_identifiers::JobName;

#[must_use]
pub fn unknown_job_names(config: &SchedulerConfig, schedule: &AdmittedSchedule) -> Vec<JobName> {
    collect_unknown_job_names(config, |name| schedule.contains_job(name.as_str()))
        .into_iter()
        .cloned()
        .collect()
}

pub(super) fn collect_unknown_job_names(
    config: &SchedulerConfig,
    is_registered: impl Fn(&JobName) -> bool,
) -> Vec<&JobName> {
    let mut unknown: Vec<&JobName> = Vec::new();
    for name in config
        .jobs
        .iter()
        .map(|job| &job.name)
        .chain(config.bootstrap_jobs.iter())
    {
        if !is_registered(name) && !unknown.contains(&name) {
            unknown.push(name);
        }
    }
    unknown
}
