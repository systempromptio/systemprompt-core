use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_scheduler::{JobConfig, SchedulerConfig, unknown_job_names};
use systemprompt_test_fixtures::fixture_admitted_without;

use crate::test_extension::{core_schedule, schedule_of};

const MCP_JOB: &str = "mcp_session_cleanup";

#[test]
fn default_bootstrap_jobs_match_built_in_inventory_names() {
    let cfg = SchedulerConfig::with_system_admin();
    assert_eq!(
        cfg.bootstrap_jobs,
        vec!["cleanup_inactive_sessions"],
        "bootstrap_jobs default must not include an irreversible deleter — database_cleanup runs \
         from its cron entry only",
    );
}

#[test]
fn every_default_bootstrap_job_is_admitted() {
    let cfg = SchedulerConfig::with_system_admin();
    let schedule = core_schedule();

    for name in &cfg.bootstrap_jobs {
        assert!(
            schedule.contains_job(name.as_str()),
            "default bootstrap job `{name}` is not in the admitted schedule; \
             SchedulerService::start would reject it with SchedulerError::UnknownJob",
        );
    }
}

#[test]
fn unknown_job_names_is_empty_for_the_built_in_config() {
    assert!(unknown_job_names(&SchedulerConfig::with_system_admin(), &core_schedule()).is_empty());
}

#[test]
fn unknown_job_names_reports_every_unregistered_name() {
    let mut cfg = SchedulerConfig::with_system_admin();
    cfg.jobs
        .push(JobConfig::new(JobName::new("access_control_sync")));
    cfg.jobs.push(JobConfig::new(JobName::new("content_sync")));

    assert_eq!(
        unknown_job_names(&cfg, &core_schedule()),
        vec!["access_control_sync", "content_sync"],
        "one bad name must not mask the others; the boot error names them all",
    );
}

#[test]
fn unknown_job_names_covers_bootstrap_jobs() {
    let mut cfg = SchedulerConfig::with_system_admin();
    cfg.bootstrap_jobs
        .push(JobName::new("publish_pipeline_typo"));

    assert_eq!(
        unknown_job_names(&cfg, &core_schedule()),
        vec!["publish_pipeline_typo"]
    );
}

#[test]
fn unknown_job_names_dedupes_a_name_in_both_lists() {
    let mut cfg = SchedulerConfig::with_system_admin();
    cfg.jobs.push(JobConfig::new(JobName::new("phantom_job")));
    cfg.bootstrap_jobs.push(JobName::new("phantom_job"));

    assert_eq!(
        unknown_job_names(&cfg, &core_schedule()),
        vec!["phantom_job"]
    );
}

#[test]
fn a_job_of_a_disabled_member_is_an_unknown_job_name() {
    let mut cfg = SchedulerConfig::with_system_admin();
    cfg.jobs.push(JobConfig::new(JobName::new(MCP_JOB)));

    assert!(
        unknown_job_names(&cfg, &core_schedule()).is_empty(),
        "with the mcp member admitted its job is known"
    );
    let without_mcp = schedule_of(&fixture_admitted_without(&[ExtensionId::new("mcp")]));
    assert_eq!(unknown_job_names(&cfg, &without_mcp), vec![MCP_JOB]);
}
