//! `infra jobs list` command.
//!
//! Lists every linked job with its owning extension. A job whose owner the
//! profile disables is listed with `owner_enabled = false`: the scheduler
//! neither schedules nor runs it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use anyhow::Result;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::JobName;
use systemprompt_loader::BootContext;
use systemprompt_scheduler::EnabledExtensions;
use systemprompt_traits::{JobRegistration, registered_jobs};

use super::types::{JobInfo, JobListOutput};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

pub(super) fn execute(ctx: &CommandContext) -> Result<CommandOutput> {
    let boot = ctx.boot()?;
    let registry = ExtensionRegistry::discover()?;
    let enabled = EnabledExtensions::resolve(&registry, &boot.profile().extensions.disabled)?;
    let configured = configured_job_names(boot);
    let mut seen_names: HashSet<&'static str> = HashSet::new();
    let mut jobs: Vec<JobInfo> = registered_jobs()
        .filter(|registration| seen_names.insert(registration.job.name()))
        .map(|registration| job_info(registration, &enabled, &configured))
        .collect();

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
