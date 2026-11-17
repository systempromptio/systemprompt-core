//! `infra jobs show` command for one job's metadata, owner and schedule.
//!
//! A job the admitted composition does not hold (a job of a member this
//! profile disables, another node's composition, or a retired job) but that
//! has a `scheduled_jobs` row is shown from that row with
//! `registered = false`: this node never claims or runs it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Args;
use systemprompt_database::pool_from_handle;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobRepository, ScheduledJob};

use super::admitted_schedule;
use super::cron_text::parse_cron_human;
use super::types::JobShowOutput;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(
        value_parser = systemprompt_extension::cli::parsers::parse_job_name,
        help = "Job name to show details for"
    )]
    pub job_name: JobName,
}

pub(super) async fn execute(args: ShowArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let schedule = admitted_schedule(host).await?;
    let registration = schedule.job(args.job_name.as_str());
    let repo = JobRepository::new(&pool_from_handle(host.db().await?)?);
    let db_job: Option<ScheduledJob> = repo.find_job(&args.job_name).await?;

    let output = match (registration, db_job) {
        (Some((owner, job)), db_job) => JobShowOutput {
            name: job.name().to_owned(),
            owner: owner.to_string(),
            description: job.description().to_owned(),
            schedule: job.schedule().to_owned(),
            schedule_human: parse_cron_human(job.schedule()),
            enabled: db_job.as_ref().map_or_else(|| job.enabled(), |j| j.enabled),
            last_run: db_job.as_ref().and_then(|j| j.last_run),
            next_run: db_job.as_ref().and_then(|j| j.next_run),
            last_status: db_job.as_ref().and_then(|j| j.last_status.clone()),
            last_error: db_job.as_ref().and_then(|j| j.last_error.clone()),
            run_count: db_job.as_ref().map_or(0, |j| j.run_count),
            registered: true,
        },
        (None, Some(row)) => JobShowOutput {
            name: row.job_name.to_string(),
            owner: String::new(),
            description: "not registered by any admitted extension".to_owned(),
            schedule_human: parse_cron_human(&row.schedule),
            schedule: row.schedule,
            enabled: row.enabled,
            last_run: row.last_run,
            next_run: row.next_run,
            last_status: row.last_status,
            last_error: row.last_error,
            run_count: row.run_count,
            registered: false,
        },
        (None, None) => anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        ),
    };

    Ok(CommandOutput::card_value(
        format!("Job: {}", args.job_name),
        &output,
    ))
}
