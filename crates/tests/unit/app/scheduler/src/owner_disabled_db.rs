//! A job whose owning member the composition leaves out is unregistered: a
//! manual run is refused by name and never runs, and a configuration naming
//! it refuses the scheduler start as an unknown job. With the member
//! admitted the same job runs and the same configuration starts.

use std::collections::HashMap;

use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_scheduler::{
    JobConfig, JobExecutionService, SchedulerConfig, SchedulerError, SchedulerService,
};
use systemprompt_test_fixtures::{
    fixture_admitted, fixture_admitted_without, test_database_url, test_db_pool,
};

use crate::test_extension::app_over;

const MCP_JOB: &str = "mcp_session_cleanup";

#[tokio::test]
async fn a_job_whose_member_is_not_admitted_is_refused_as_unregistered() {
    let without = fixture_admitted_without(&[
        ExtensionId::new("mcp"),
        ExtensionId::new("mcp-http"),
        ExtensionId::new("agent"),
    ]);
    let (ctx, schedule) = app_over(&test_database_url(), without).await;
    assert!(!schedule.contains_job(MCP_JOB));

    let service = JobExecutionService::new(
        ctx,
        systemprompt_manifest::SchedulerConfig::with_system_admin(),
        schedule,
    )
    .expect("job execution service");
    let report = service
        .run_job(&JobName::new(MCP_JOB), &HashMap::new())
        .await;

    assert!(!report.success, "a job of a member left out must not run");
    let message = report.message.unwrap_or_default();
    assert!(
        message.contains(MCP_JOB) && message.contains("not registered by any admitted extension"),
        "the refusal names the job as unregistered: {message}"
    );
}

#[tokio::test]
async fn the_same_job_runs_with_its_member_admitted() {
    let (ctx, schedule) = app_over(&test_database_url(), fixture_admitted()).await;
    let service = JobExecutionService::new(
        ctx,
        systemprompt_manifest::SchedulerConfig::with_system_admin(),
        schedule,
    )
    .expect("job execution service");
    let report = service
        .run_job(&JobName::new(MCP_JOB), &HashMap::new())
        .await;

    assert!(
        report.success,
        "an admitted member's job runs: {:?}",
        report.message
    );
}

fn naming_the_mcp_job() -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: vec![JobConfig::new(JobName::new(MCP_JOB))],
        bootstrap_jobs: Vec::new(),
        distributed_lock: false,
        unlisted_jobs: UnlistedJobs::Warn,
    }
}

#[tokio::test]
async fn a_configuration_naming_a_left_out_members_job_refuses_the_start() {
    let pool = test_db_pool().await;
    let without = fixture_admitted_without(&[
        ExtensionId::new("mcp"),
        ExtensionId::new("mcp-http"),
        ExtensionId::new("agent"),
    ]);
    let (ctx, schedule) = app_over(&test_database_url(), without).await;

    let err = SchedulerService::new(naming_the_mcp_job(), &pool, ctx, schedule)
        .expect("scheduler")
        .start()
        .await
        .expect_err("an unknown job refuses the start");

    let SchedulerError::UnknownJob { names } = &err else {
        panic!("expected UnknownJob, got {err}");
    };
    assert!(names.contains(MCP_JOB), "{names}");
}

#[tokio::test]
async fn the_same_configuration_starts_with_the_member_admitted() {
    let pool = test_db_pool().await;
    let (ctx, schedule) = app_over(&test_database_url(), fixture_admitted()).await;

    let startup = SchedulerService::new(naming_the_mcp_job(), &pool, ctx, schedule)
        .expect("scheduler")
        .start()
        .await
        .expect("an admitted member's job starts");
    if let Some(handle) = startup.handle {
        handle.shutdown().await.expect("scheduler drains");
    }
}
