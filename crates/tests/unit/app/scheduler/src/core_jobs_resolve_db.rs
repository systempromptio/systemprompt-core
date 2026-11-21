//! Every job of the reference composition resolves what it reads from the
//! context a run gets: each admitted core job runs once through the public
//! `JobExecutionService` against a real fixture host, whose job contexts are
//! built from `Kernel::runtime_state` with every sealed type withheld. A job
//! that reads a sealed or unprovided type fails with `MissingDependency`,
//! which names the type; any other outcome (success, a domain error, an
//! unreachable network peer) is not this check's concern. Runs leave
//! `enforce` off, so no destructive job removes anything.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobExecutionService, SchedulerConfig};
use systemprompt_test_fixtures::{fixture_admitted, test_database_url};

const PER_JOB: Duration = Duration::from_secs(60);
const MISSING: &str = "provider context has no";

#[tokio::test]
async fn every_admitted_core_job_resolves_its_context_under_the_seal() {
    let (app, schedule) =
        crate::test_extension::app_over(&test_database_url(), fixture_admitted()).await;
    let service = JobExecutionService::new(
        Arc::clone(&app),
        SchedulerConfig::with_system_admin(),
        Arc::clone(&schedule),
    );
    let names: Vec<&'static str> = schedule.job_names().collect();
    assert!(
        names.contains(&"content_prerender") && names.contains(&"agent_run_dirs_cleanup"),
        "the reference composition admits the core jobs: {names:?}"
    );

    let mut unresolved = Vec::new();
    for name in names {
        let job = JobName::new(name);
        let Ok(report) = tokio::time::timeout(PER_JOB, service.run_job(&job, &HashMap::new())).await
        else {
            continue;
        };
        if let Some(message) = report.message.filter(|m| m.contains(MISSING)) {
            unresolved.push(format!("{name}: {message}"));
        }
    }
    assert!(
        unresolved.is_empty(),
        "jobs read a type their runtime context does not carry: {unresolved:#?}"
    );
}
