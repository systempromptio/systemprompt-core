use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::Job;

use super::types::JobEnableOutput;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct DisableArgs {
    #[arg(help = "Job name to disable")]
    pub job_name: String,
}

pub async fn execute(args: DisableArgs) -> Result<CommandResult<JobEnableOutput>> {
    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == args.job_name)
        .copied();

    if job.is_none() {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    }

    let ctx = Arc::new(AppContext::new().await?);
    let pool = ctx.db_pool().write_pool_arc()?;

    sqlx::query!(
        "UPDATE scheduled_jobs SET enabled = false, updated_at = NOW() WHERE job_name = $1",
        args.job_name
    )
    .execute(&*pool)
    .await
    .context("Failed to disable job")?;

    let output = JobEnableOutput {
        job_name: args.job_name.clone(),
        enabled: false,
        message: format!("Job '{}' has been disabled", args.job_name),
    };

    Ok(CommandResult::text(output).with_title("Job Disabled"))
}
