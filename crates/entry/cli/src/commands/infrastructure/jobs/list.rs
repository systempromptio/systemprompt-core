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
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut jobs: Vec<JobInfo> = Vec::new();

    for job in registry.all_jobs() {
        if seen_names.insert(job.name().to_owned()) {
            jobs.push(JobInfo {
                name: job.name().to_owned(),
                description: job.description().to_owned(),
                schedule: job.schedule().to_owned(),
                enabled: job.enabled(),
            });
        }
    }

    for job in inventory::iter::<&'static dyn Job> {
        if seen_names.insert(job.name().to_owned()) {
            jobs.push(JobInfo {
                name: job.name().to_owned(),
                description: job.description().to_owned(),
                schedule: job.schedule().to_owned(),
                enabled: job.enabled(),
            });
        }
    }

    let total = jobs.len();
    let output = JobListOutput { jobs, total };

    CommandOutput::table_of(
        vec!["name", "description", "schedule", "enabled"],
        &output.jobs,
    )
    .with_title("Available Jobs")
}
