use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::Job;

use super::types::JobEnableOutput;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct EnableArgs {
    #[arg(help = "Job name to enable")]
    pub job_name: String,
}

pub async fn execute(args: EnableArgs) -> Result<CommandResult<JobEnableOutput>> {
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
        "UPDATE scheduled_jobs SET enabled = true, updated_at = NOW() WHERE job_name = $1",
        args.job_name
    )
    .execute(&*pool)
    .await
    .context("Failed to enable job")?;

    let output = JobEnableOutput {
        job_name: args.job_name.clone(),
        enabled: true,
        message: format!("Job '{}' has been enabled", args.job_name),
    };

    Ok(CommandResult::text(output).with_title("Job Enabled"))
}
