//! A manual run of a job whose owning member the composition leaves out is
//! refused by name as unregistered, and never runs; the same job with the
//! member admitted runs.

use std::collections::HashMap;

use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_scheduler::JobExecutionService;
use systemprompt_test_fixtures::{fixture_admitted, fixture_admitted_without, test_database_url};

use crate::test_extension::app_over;

const MCP_JOB: &str = "mcp_session_cleanup";

#[tokio::test]
async fn a_job_whose_member_is_not_admitted_is_refused_as_unregistered() {
    let without = fixture_admitted_without(&[ExtensionId::new("mcp")]);
    let (ctx, schedule) = app_over(&test_database_url(), without).await;
    assert!(!schedule.contains_job(MCP_JOB));

    let service = JobExecutionService::new(
        ctx,
        systemprompt_manifest::SchedulerConfig::with_system_admin(),
        schedule,
    );
    let report = service.run_job(&JobName::new(MCP_JOB), &HashMap::new()).await;

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
    );
    let report = service.run_job(&JobName::new(MCP_JOB), &HashMap::new()).await;

    assert!(report.success, "an admitted member's job runs: {:?}", report.message);
}
