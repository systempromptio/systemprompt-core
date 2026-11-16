//! `infra jobs list` command.
//!
//! Lists every job of the admitted composition with its owning member. A
//! `scheduled_jobs` row whose job is not admitted (a job of a member this
//! profile disables, another node's composition, or a retired job) is listed
//! with `registered = false`: this node never claims or runs it, and
//! `infra jobs prune` is the command that removes it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use crate::kernel::KernelHost;

use anyhow::Result;
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_scheduler::{JobRepository, ScheduledJob};
use systemprompt_traits::Job;

use super::admitted_schedule;
use super::types::{JobInfo, JobListOutput};
use crate::commands::shared::services_config;

pub(super) async fn execute(host: &dyn KernelHost) -> Result<CommandOutput> {
    let schedule = admitted_schedule(host).await?;
    let configured = configured_job_names(host);
    let mut jobs: Vec<JobInfo> = schedule
        .jobs()
        .iter()
        .map(|(owner, job)| job_info(owner, job.as_ref(), &configured))
        .collect();
    let rows = JobRepository::new(&pool_from_handle(host.db().await?)?)
        .list_jobs()
        .await?;
    jobs.extend(
        rows.into_iter()
            .filter(|row| !schedule.contains_job(row.job_name.as_str()))
            .map(unregistered_row),
    );

    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    let total = jobs.len();
    let output = JobListOutput { jobs, total };

    Ok(CommandOutput::table_of(
        vec![
            "name",
            "owner",
            "description",
            "schedule",
            "enabled",
            "scheduled",
            "registered",
        ],
        &output.jobs,
    )
    .with_title("Available Jobs"))
}

fn job_info(owner: &ExtensionId, job: &dyn Job, configured: &HashSet<JobName>) -> JobInfo {
    JobInfo {
        name: job.name().to_owned(),
        owner: owner.to_string(),
        description: job.description().to_owned(),
        schedule: job.schedule().to_owned(),
        enabled: job.enabled(),
        scheduled: configured.contains(job.name()),
        registered: true,
    }
}

fn unregistered_row(row: ScheduledJob) -> JobInfo {
    JobInfo {
        name: row.job_name.to_string(),
        owner: String::new(),
        description: "not registered by any admitted extension".to_owned(),
        schedule: row.schedule,
        enabled: row.enabled,
        scheduled: false,
        registered: false,
    }
}

fn configured_job_names(host: &dyn KernelHost) -> HashSet<JobName> {
    let Ok(config) = services_config(host) else {
        return HashSet::new();
    };
    let Some(scheduler) = config.scheduler else {
        return HashSet::new();
    };
    scheduler
        .jobs
        .iter()
        .filter(|job| job.enabled)
        .map(|job| job.name.clone())
        .chain(scheduler.bootstrap_jobs.iter().cloned())
        .collect()
}
