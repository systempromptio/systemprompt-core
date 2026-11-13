//! `infra jobs show` command for one job's metadata, owner and schedule.
//!
//! `owner_enabled = false` marks a job whose owning extension the profile
//! disables; the scheduler neither schedules nor runs it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{EnabledExtensions, JobRepository, ScheduledJob};

use super::cron_text::parse_cron_human;
use super::types::JobShowOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(
        value_parser = crate::shared::parse_job_name,
        help = "Job name to show details for"
    )]
    pub job_name: JobName,
}

pub(super) async fn execute(args: ShowArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let registration =
        systemprompt_traits::registered_jobs().find(|r| r.job.name() == args.job_name);

    let Some(registration) = registration else {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    };

    let job = registration.job;
    let app = ctx.app_context().await?;
    let owner_enabled = EnabledExtensions::of(&app)?.contains(registration.owner);
    let repo = JobRepository::new(app.db_pool());

    let db_job: Option<ScheduledJob> = repo.find_job(&args.job_name).await?;

    let output = JobShowOutput {
        name: job.name().to_owned(),
        owner: registration.owner.to_owned(),
        owner_enabled,
        description: job.description().to_owned(),
        schedule: job.schedule().to_owned(),
        schedule_human: parse_cron_human(job.schedule()),
        enabled: db_job.as_ref().map_or_else(|| job.enabled(), |j| j.enabled),
        last_run: db_job.as_ref().and_then(|j| j.last_run),
        next_run: db_job.as_ref().and_then(|j| j.next_run),
        last_status: db_job.as_ref().and_then(|j| j.last_status.clone()),
        last_error: db_job.as_ref().and_then(|j| j.last_error.clone()),
        run_count: db_job.as_ref().map_or(0, |j| j.run_count),
    };

    Ok(CommandOutput::card_value(
        format!("Job: {}", args.job_name),
        &output,
    ))
}
