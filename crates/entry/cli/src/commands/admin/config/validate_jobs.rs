//! Cross-checks the merged scheduler config against the registered job
//! catalog for `admin config validate`.
//!
//! A configured job name no `submit_job!` registers is always an error; with
//! `--strict`, so is a registered job with no `scheduler.jobs` entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::types::ConfigFileInfo;
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_scheduler::SchedulerConfig;

pub(super) fn scheduler_catalog_error(
    boot: &BootContext,
    services: Option<&ServicesConfig>,
    files: &[ConfigFileInfo],
    strict: bool,
) -> Option<String> {
    if !files
        .iter()
        .any(|file| file.section == "scheduler" && file.error.is_none())
    {
        return None;
    }
    let scheduler = services?
        .scheduler
        .clone()
        .unwrap_or_else(SchedulerConfig::with_system_admin);
    unknown_jobs_message(&scheduler).or_else(|| {
        strict
            .then(|| unlisted_jobs_message(&scheduler, boot))
            .flatten()
    })
}

pub fn unlisted_jobs_message(config: &SchedulerConfig, boot: &BootContext) -> Option<String> {
    let unlisted = systemprompt_scheduler::unlisted_job_names(config, boot);
    if unlisted.is_empty() {
        return None;
    }
    Some(format!(
        "registered job(s) with no scheduler.jobs entry: {}. Add an entry for each (enabled: \
         false keeps one off the schedule).",
        unlisted
            .iter()
            .map(systemprompt_identifiers::JobName::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub fn unknown_jobs_message(config: &SchedulerConfig) -> Option<String> {
    let unknown = systemprompt_scheduler::unknown_job_names(config);
    if unknown.is_empty() {
        return None;
    }
    Some(format!(
        "job(s) not registered via submit_job!: {}. Run `systemprompt infra jobs list` to see \
         every registered job.",
        unknown.join(", ")
    ))
}
