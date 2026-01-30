use std::collections::HashSet;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_traits::Job;

use super::types::{JobInfo, JobListOutput};
use crate::shared::{CommandResult, RenderingHints};

pub fn execute() -> CommandResult<JobListOutput> {
    let registry = ExtensionRegistry::discover();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut jobs: Vec<JobInfo> = Vec::new();

    for job in registry.all_jobs() {
        if seen_names.insert(job.name().to_string()) {
            jobs.push(JobInfo {
                name: job.name().to_string(),
                description: job.description().to_string(),
                schedule: job.schedule().to_string(),
                enabled: job.enabled(),
            });
        }
    }

    for job in inventory::iter::<&'static dyn Job> {
        if seen_names.insert(job.name().to_string()) {
            jobs.push(JobInfo {
                name: job.name().to_string(),
                description: job.description().to_string(),
                schedule: job.schedule().to_string(),
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
                "name".to_string(),
                "description".to_string(),
                "schedule".to_string(),
                "enabled".to_string(),
            ]),
            ..Default::default()
        })
}
