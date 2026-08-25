//! `infra jobs list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_traits::Job;

use super::types::{JobInfo, JobListOutput};
use crate::shared::CommandOutput;

pub(super) fn execute() -> CommandOutput {
    let registry = ExtensionRegistry::discover().unwrap_or_else(|e| {
        tracing::error!(error = %e, "extension dependency cycle; using empty registry");
        ExtensionRegistry::new()
    });
    let configured = configured_job_names();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut jobs: Vec<JobInfo> = Vec::new();

    for job in registry.all_jobs() {
        if seen_names.insert(job.name().to_owned()) {
            jobs.push(job_info(job.name(), job.as_ref(), &configured));
        }
    }

    for job in inventory::iter::<&'static dyn Job> {
        if seen_names.insert(job.name().to_owned()) {
            jobs.push(job_info(job.name(), *job, &configured));
        }
    }

    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    let total = jobs.len();
    let output = JobListOutput { jobs, total };

    CommandOutput::table_of(
        vec!["name", "description", "schedule", "enabled", "scheduled"],
        &output.jobs,
    )
    .with_title("Available Jobs")
}

fn job_info(name: &str, job: &dyn Job, configured: &HashSet<String>) -> JobInfo {
    JobInfo {
        name: name.to_owned(),
        description: job.description().to_owned(),
        schedule: job.schedule().to_owned(),
        enabled: job.enabled(),
        scheduled: configured.contains(name),
    }
}

fn configured_job_names() -> HashSet<String> {
    let Ok(config) = systemprompt_loader::ConfigLoader::load() else {
        return HashSet::new();
    };
    let Some(scheduler) = config.scheduler else {
        return HashSet::new();
    };
    scheduler
        .jobs
        .iter()
        .filter(|job| job.enabled)
        .map(|job| job.name.clone())
        .chain(scheduler.bootstrap_jobs.iter().cloned())
        .collect()
}
