//! `infra jobs enable` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Args;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::JobRepository;

use super::admitted_schedule;
use super::types::JobEnableOutput;
use crate::commands::shared::app_context;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct EnableArgs {
    #[arg(value_parser = systemprompt_extension::cli::parsers::parse_job_name, help = "Job name to enable")]
    pub job_name: JobName,
}

pub(super) async fn execute(args: EnableArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    if !admitted_schedule(host)
        .await?
        .contains_job(args.job_name.as_str())
    {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    }

    let app = app_context(host)
        .await?;
    let repo = JobRepository::new(app.db_pool());
    repo.set_enabled(&args.job_name, true)
        .await
        .context("Failed to enable job")?;

    let output = JobEnableOutput {
        job_name: args.job_name.clone(),
        enabled: true,
        message: format!("Job '{}' has been enabled", args.job_name),
    };

    Ok(CommandOutput::card_value("Job Enabled", &output))
}
