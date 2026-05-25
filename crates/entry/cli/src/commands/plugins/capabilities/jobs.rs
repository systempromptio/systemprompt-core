use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{JobWithExtension, JobsListOutput};
use crate::shared::CommandResult;

#[derive(Debug, Clone, Args)]
pub struct JobsArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,

    #[arg(long, help = "Show only enabled jobs")]
    pub enabled: bool,
}

pub(crate) fn execute(args: &JobsArgs, _config: &CliConfig) -> CommandResult<JobsListOutput> {
    let registry = discover_registry();

    let jobs: Vec<JobWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.jobs()
                .iter()
                .filter_map(|job| {
                    if args.enabled && !job.enabled() {
                        return None;
                    }

                    Some(JobWithExtension {
                        extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                        extension_name: ext.name().to_owned(),
                        job_name: job.name().to_owned(),
                        schedule: job.schedule().to_owned(),
                        enabled: job.enabled(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = jobs.len();

    let output = JobsListOutput { jobs, total };

    CommandResult::table(output)
        .with_title("Jobs Across Extensions")
        .with_columns(vec![
            "extension_id".to_owned(),
            "job_name".to_owned(),
            "schedule".to_owned(),
            "enabled".to_owned(),
        ])
}
