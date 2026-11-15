//! `infra jobs list` command.
//!
//! Lists every linked job with its owning extension. A job whose owner the
//! profile disables is listed with `owner_enabled = false`: the scheduler
//! neither schedules nor runs it. A `scheduled_jobs` row whose job this binary
//! does not link (another node's release or composition, or a retired job) is
//! listed with `registered = false`: this node never claims or runs it, and
//! `infra jobs prune` is the command that removes it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use anyhow::Result;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::JobName;
use systemprompt_loader::BootContext;
use systemprompt_scheduler::{EnabledExtensions, JobRepository, ScheduledJob};
use systemprompt_traits::{JobRegistration, registered_jobs};

use super::types::{JobInfo, JobListOutput};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

pub(super) async fn execute(ctx: &CommandContext) -> Result<CommandOutput> {
    let boot = ctx.boot()?;
    let registry = ExtensionRegistry::discover()?;
    let enabled = EnabledExtensions::resolve(&registry, &boot.profile().extensions.disabled)?;
    let configured = configured_job_names(boot);
    let mut seen_names: HashSet<&'static str> = HashSet::new();
    let mut jobs: Vec<JobInfo> = registered_jobs()
        .filter(|registration| seen_names.insert(registration.job.name()))
        .map(|registration| job_info(registration, &enabled, &configured))
        .collect();
    let rows = JobRepository::new(&ctx.db_pool().await?).list_jobs().await?;
    jobs.extend(
        rows.into_iter()
            .filter(|row| !seen_names.contains(row.job_name.as_str()))
            .map(unregistered_row),
    );

    jobs.sort_by(|a, b| a.name.cmp(&b.name));
    let total = jobs.len();
    let output = JobListOutput { jobs, total };

    Ok(CommandOutput::table_of(
        vec![
            "name",
            "owner",
            "owner_enabled",
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

fn job_info(
    registration: &JobRegistration,
    enabled: &EnabledExtensions,
    configured: &HashSet<JobName>,
) -> JobInfo {
    let job = registration.job;
    JobInfo {
        name: job.name().to_owned(),
        owner: registration.owner.to_owned(),
        owner_enabled: enabled.contains(registration.owner),
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
        owner_enabled: false,
        description: "not registered on this node".to_owned(),
        schedule: row.schedule,
        enabled: row.enabled,
        scheduled: false,
        registered: false,
    }
}

fn configured_job_names(boot: &BootContext) -> HashSet<JobName> {
    let Ok(config) = boot.load_services_config() else {
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
