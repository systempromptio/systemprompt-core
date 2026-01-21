use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{JobRepository, ScheduledJob};
use systemprompt_traits::Job;

use super::helpers::parse_cron_human;
use super::types::JobShowOutput;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Job name to show details for")]
    pub job_name: String,
}

pub async fn execute(args: ShowArgs) -> Result<CommandResult<JobShowOutput>> {
    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == args.job_name)
        .copied();

    let Some(job) = job else {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    };

    let ctx = Arc::new(AppContext::new().await?);
    let repo = JobRepository::new(ctx.db_pool())?;

    let db_job: Option<ScheduledJob> = repo.find_job(&args.job_name).await?;

    let output = JobShowOutput {
        name: job.name().to_string(),
        description: job.description().to_string(),
        schedule: job.schedule().to_string(),
        schedule_human: parse_cron_human(job.schedule()),
        enabled: db_job.as_ref().map_or_else(|| job.enabled(), |j| j.enabled),
        last_run: db_job.as_ref().and_then(|j| j.last_run),
        next_run: db_job.as_ref().and_then(|j| j.next_run),
        last_status: db_job.as_ref().and_then(|j| j.last_status.clone()),
        last_error: db_job.as_ref().and_then(|j| j.last_error.clone()),
        run_count: db_job.as_ref().map_or(0, |j| j.run_count),
    };

    Ok(CommandResult::card(output).with_title(format!("Job: {}", args.job_name)))
}
