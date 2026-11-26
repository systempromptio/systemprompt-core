//! Cross-checks the merged scheduler config against the admitted job schedule
//! for `admin config validate`.
//!
//! A configured job name no admitted member contributes is always an error;
//! with `--strict`, so is an admitted job with no `scheduler.jobs` entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::types::ConfigFileInfo;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_scheduler::SchedulerConfig;

pub(super) fn scheduler_catalog_error(
    boot: &BootContext,
    schedule: &AdmittedSchedule,
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
    let scheduler = services?.scheduler.clone().unwrap_or_else(|| {
        SchedulerConfig::with_system_admin_admitted(|name| schedule.contains_job(name))
    });
    unknown_jobs_message(&scheduler, schedule).or_else(|| {
        strict
            .then(|| unlisted_jobs_message(&scheduler, boot, schedule))
            .flatten()
    })
}

pub fn unlisted_jobs_message(
    config: &SchedulerConfig,
    boot: &BootContext,
    schedule: &AdmittedSchedule,
) -> Option<String> {
    let unlisted = systemprompt_scheduler::unlisted_job_names(config, boot, schedule);
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

pub fn unknown_jobs_message(
    config: &SchedulerConfig,
    schedule: &AdmittedSchedule,
) -> Option<String> {
    let unknown = systemprompt_scheduler::unknown_job_names(config, schedule);
    if unknown.is_empty() {
        return None;
    }
    Some(format!(
        "job(s) no enabled extension contributes: {}. Run `systemprompt infra jobs list` to see \
         every admitted job.",
        unknown
            .iter()
            .map(systemprompt_identifiers::JobName::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
