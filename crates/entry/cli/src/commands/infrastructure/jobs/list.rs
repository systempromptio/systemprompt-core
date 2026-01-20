use systemprompt_traits::Job;

use super::types::{JobInfo, JobListOutput};
use crate::shared::{CommandResult, RenderingHints};

pub fn execute() -> CommandResult<JobListOutput> {
    let jobs: Vec<JobInfo> = inventory::iter::<&'static dyn Job>
        .into_iter()
        .map(|job| JobInfo {
            name: job.name().to_string(),
            description: job.description().to_string(),
            schedule: job.schedule().to_string(),
            enabled: job.enabled(),
        })
        .collect();

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
