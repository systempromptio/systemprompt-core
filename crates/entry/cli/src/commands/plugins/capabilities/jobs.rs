//! `plugins capabilities jobs` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;
use systemprompt_extension::BuiltState;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_identifiers::JobName;

use crate::commands::plugins::composition;
use crate::commands::plugins::types::{JobWithExtension, JobsListOutput};

#[derive(Debug, Clone, Args)]
pub struct JobsArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,

    #[arg(long, help = "Show only enabled jobs")]
    pub enabled: bool,
}

pub fn execute(args: &JobsArgs, built: &BuiltState) -> CommandOutput {
    let jobs: Vec<JobWithExtension> = composition::matching(built, args.extension.as_deref())
        .flat_map(|member| {
            member
                .sink
                .jobs()
                .iter()
                .map(systemprompt_extension::contribute::ContributedJob::job)
                .filter(|job| !args.enabled || job.enabled())
                .map(|job| JobWithExtension {
                    extension_id: member.owner.clone(),
                    extension_name: member.metadata.name.to_owned(),
                    job_name: JobName::new(job.name()),
                    schedule: job.schedule().to_owned(),
                    enabled: job.enabled(),
                })
        })
        .collect();

    let total = jobs.len();
    let output = JobsListOutput { jobs, total };

    CommandOutput::table_of(
        vec!["extension_id", "job_name", "schedule", "enabled"],
        &output.jobs,
    )
    .with_title("Jobs Across Extensions")
}
