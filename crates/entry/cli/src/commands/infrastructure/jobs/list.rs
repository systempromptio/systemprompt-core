use std::collections::HashSet;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_traits::Job;

use super::types::{JobInfo, JobListOutput};
use crate::shared::{CommandResult, RenderingHints};

pub(super) fn execute() -> CommandResult<JobListOutput> {
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

    CommandResult::table(output)
        .with_title("Available Jobs")
        .with_hints(RenderingHints {
            columns: Some(vec![
                "name".to_owned(),
                "description".to_owned(),
                "schedule".to_owned(),
                "enabled".to_owned(),
            ]),
            ..Default::default()
        })
}
