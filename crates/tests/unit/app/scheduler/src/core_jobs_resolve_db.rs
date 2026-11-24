//! Every job of the reference composition resolves what it reads from the
//! context a run gets: each admitted core job runs once through the public
//! `JobExecutionService` against a real fixture host, whose job contexts are
//! built from `Kernel::runtime_state` with every sealed type withheld.
//!
//! The check is on the typed error: a job whose run returned a
//! `ProviderError` that is, or has anywhere in its source chain, a
//! `MissingDependency` read a type its context does not carry. A job that
//! did not finish within its bound fails the check, as does a job refused
//! before its `execute` ran; any other outcome (success, a domain error, an
//! unreachable network peer) is not this check's concern. Runs leave
//! `enforce` off, so no destructive job removes anything.
//!
//! Only jobs are covered here: configuration planes report their failures as
//! rendered text (`PlaneReport::error`), and service runners are not run by
//! the scheduler.

use std::collections::HashMap;
use std::error::Error as _;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::MissingDependency;
use systemprompt_identifiers::JobName;
use systemprompt_provider_contracts::ProviderError;
use systemprompt_scheduler::{JobExecutionService, SchedulerConfig};
use systemprompt_test_fixtures::{DisposableDb, fixture_admitted};

const PER_JOB: Duration = Duration::from_secs(60);

fn missing_dependency(error: &ProviderError) -> Option<&MissingDependency> {
    if let ProviderError::MissingDependency(missing) = error {
        return Some(missing);
    }
    let mut source = error.source();
    while let Some(cause) = source {
        if let Some(missing) = cause.downcast_ref::<MissingDependency>() {
            return Some(missing);
        }
        if let Some(ProviderError::MissingDependency(missing)) =
            cause.downcast_ref::<ProviderError>()
        {
            return Some(missing);
        }
        source = cause.source();
    }
    None
}

#[test]
fn a_wrapped_missing_dependency_is_found_through_the_source_chain() {
    let direct = ProviderError::from(MissingDependency::of::<String>());
    assert!(missing_dependency(&direct).is_some());
    let wrapped = ProviderError::Internal(Box::new(MissingDependency::of::<String>()));
    assert!(missing_dependency(&wrapped).is_some());
    let other = ProviderError::SourceNotFound {
        source_id: systemprompt_identifiers::SourceId::new("row"),
    };
    assert!(missing_dependency(&other).is_none());
}

#[tokio::test]
async fn every_admitted_core_job_resolves_its_context_under_the_seal() {
    let database = DisposableDb::with_schema("core_jobs_resolve").await;
    let (app, schedule) = crate::test_extension::app_over(database.url(), fixture_admitted()).await;
    let service = JobExecutionService::new(
        Arc::clone(&app),
        SchedulerConfig::with_system_admin(),
        Arc::clone(&schedule),
    );
    let names: Vec<&'static str> = schedule.job_names().collect();
    assert!(
        names.contains(&"managed_inventory_refresh") && names.contains(&"agent_run_dirs_cleanup"),
        "the reference composition admits the core jobs: {names:?}"
    );

    let mut failures = Vec::new();
    for name in names {
        let job = JobName::new(name);
        let Ok(report) =
            tokio::time::timeout(PER_JOB, service.run_job(&job, &HashMap::new())).await
        else {
            failures.push(format!("{name}: did not finish within {PER_JOB:?}"));
            continue;
        };
        if !report.executed {
            failures.push(format!(
                "{name}: refused before execute: {:?}",
                report.message
            ));
            continue;
        }
        if let Some(missing) = report.error.as_deref().and_then(missing_dependency) {
            failures.push(format!("{name}: {missing}"));
        }
    }
    assert!(
        failures.is_empty(),
        "jobs did not resolve their runtime context: {failures:#?}"
    );
}
