//! `scheduler.unlisted_jobs`: a registered job the configuration never names
//! is warned about, scheduled on its declared default, or refused at start.
//! The `default` cases put real jobs on the cron loop and write
//! `scheduled_jobs`, so these tests join the serialized `scheduler-jobs-db`
//! nextest group.

use std::sync::Arc;

use systemprompt_identifiers::JobName;
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_scheduler::{
    JobConfig, SchedulerConfig, SchedulerError, SchedulerRepository, SchedulerService,
    unlisted_job_names,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_traits::Job;

use crate::test_jobs::{FAILING_JOB, WORKING_JOB};

const DEFAULTED_JOB: &str = "trace_facts_rollup";

fn config(mode: UnlistedJobs, jobs: Vec<JobConfig>) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs,
        bootstrap_jobs: Vec::new(),
        distributed_lock: false,
        unlisted_jobs: mode,
    }
}

fn names(config: &SchedulerConfig) -> Vec<String> {
    unlisted_job_names(config, &ensure_test_bootstrap().boot)
        .into_iter()
        .map(|name| name.as_str().to_owned())
        .collect()
}

#[test]
fn unlisted_jobs_exclude_named_and_disabled_jobs() {
    let all = names(&config(UnlistedJobs::Warn, Vec::new()));
    assert!(all.iter().any(|n| n == DEFAULTED_JOB));
    for disabled in [WORKING_JOB, FAILING_JOB] {
        assert!(
            !all.iter().any(|n| n == disabled),
            "a job disabled in code is never unlisted"
        );
    }

    let named = names(&config(
        UnlistedJobs::Warn,
        vec![JobConfig::new(JobName::new(DEFAULTED_JOB)).disabled()],
    ));
    assert!(
        !named.iter().any(|n| n == DEFAULTED_JOB),
        "an entry with enabled: false still lists the job"
    );
}

#[test]
fn the_read_model_rollups_declare_a_default_schedule() {
    let declared: Vec<&str> = inventory::iter::<&'static dyn Job>
        .into_iter()
        .filter(|job| job.default_schedule().is_some())
        .map(|job| job.name())
        .collect();
    for rollup in [
        "analytics_event_link_facts_rollup",
        "session_bot_type_rollup",
        "ai_request_tool_flag_rollup",
        "tool_row_facts_rollup",
        "log_summary_rollup",
        "trace_facts_rollup",
        "user_context_counters_rollup",
        "backfill_rollups",
    ] {
        assert!(
            declared.contains(&rollup),
            "{rollup} must declare a default"
        );
    }
    for job in inventory::iter::<&'static dyn Job> {
        if let Some(schedule) = job.default_schedule() {
            assert_eq!(schedule, job.schedule(), "{}", job.name());
        }
    }
}

#[tokio::test]
async fn error_mode_refuses_to_start_and_names_the_jobs() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    let svc = SchedulerService::new(
        config(UnlistedJobs::Error, Vec::new()),
        Arc::clone(&pool),
        app_ctx,
    );

    let err = svc.start().await.expect_err("unlisted jobs must fail boot");
    let SchedulerError::UnlistedJobs { names } = &err else {
        panic!("unexpected error: {err}");
    };
    assert!(names.contains(DEFAULTED_JOB), "{names}");
    assert!(!names.contains(FAILING_JOB), "{names}");
}

#[tokio::test]
async fn default_mode_schedules_jobs_with_a_declared_default() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    let svc = SchedulerService::new(
        config(UnlistedJobs::Default, Vec::new()),
        Arc::clone(&pool),
        app_ctx,
    );

    let startup = svc.start().await.expect("scheduler starts");
    let handle = startup.handle.expect("scheduler running");
    handle.shutdown().await.expect("scheduler drains");

    assert!(startup.scheduled >= 8, "scheduled {}", startup.scheduled);
    let row = SchedulerRepository::new(&pool)
        .find_job(&JobName::new(DEFAULTED_JOB))
        .await
        .expect("find_job")
        .expect("a defaulted job is registered like a configured one");
    assert!(row.enabled);
}

#[tokio::test]
async fn warn_mode_schedules_nothing_it_was_not_told_to() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    let svc = SchedulerService::new(
        config(UnlistedJobs::Warn, Vec::new()),
        Arc::clone(&pool),
        app_ctx,
    );

    let startup = svc.start().await.expect("scheduler starts");
    if let Some(handle) = startup.handle {
        handle.shutdown().await.expect("scheduler drains");
    }
    assert_eq!(startup.scheduled, 0);
}
