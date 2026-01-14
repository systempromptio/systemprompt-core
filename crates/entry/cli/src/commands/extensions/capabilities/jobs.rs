use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use crate::commands::extensions::types::{JobWithExtension, JobsListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct JobsArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,

    #[arg(long, help = "Show only enabled jobs")]
    pub enabled: bool,
}

pub fn execute(args: &JobsArgs, _config: &CliConfig) -> CommandResult<JobsListOutput> {
    let registry = ExtensionRegistry::discover();

    let jobs: Vec<JobWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| {
            args.extension
                .as_ref()
                .is_none_or( |f| ext.id().contains(f))
        })
        .flat_map(|ext| {
            ext.jobs().iter().filter_map(|job| {
                if args.enabled && !job.enabled() {
                    return None;
                }

                Some(JobWithExtension {
                    extension_id: ext.id().to_string(),
                    extension_name: ext.name().to_string(),
                    job_name: job.name().to_string(),
                    schedule: job.schedule().to_string(),
                    enabled: job.enabled(),
                })
            }).collect::<Vec<_>>()
        })
        .collect();

    let total = jobs.len();

    let output = JobsListOutput { jobs, total };

    CommandResult::table(output)
        .with_title("Jobs Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "job_name".to_string(),
            "schedule".to_string(),
            "enabled".to_string(),
        ])
}
