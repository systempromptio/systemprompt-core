//! A job that declares a `db_budget` is dispatched with a `BudgetedDb` whose
//! transactions carry that budget as `SET LOCAL` settings.

use std::sync::Arc;

use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobStatus, SchedulerConfig, SchedulerRepository, SchedulerService};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};

use crate::test_jobs::{BUDGETED_JOB, BUDGETED_JOB_SETTINGS};

#[tokio::test]
async fn a_budgeted_job_runs_its_transactions_under_the_declared_limits() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let app_ctx = test_app_context(&pool, &url);
    let repo = SchedulerRepository::new(&pool);
    repo.upsert_job(&JobName::new(BUDGETED_JOB), "0 0 * * * *", true)
        .await
        .expect("seed scheduled_jobs row");

    let config = SchedulerConfig {
        enabled: true,
        jobs: Vec::new(),
        bootstrap_jobs: vec![JobName::new(BUDGETED_JOB)],
        distributed_lock: false,
        unlisted_jobs: systemprompt_manifest::services::UnlistedJobs::Warn,
    };
    SchedulerService::new(config, Arc::clone(&pool), app_ctx)
        .run_bootstrap_jobs(None)
        .await
        .expect("bootstrap dispatch");

    let settings = BUDGETED_JOB_SETTINGS.lock().expect("lock").clone();
    assert_eq!(
        settings,
        vec![("7s".to_owned(), "3s".to_owned(), "2MB".to_owned())],
        "the job's transaction must carry its declared budget"
    );
    let row = repo
        .find_job(&JobName::new(BUDGETED_JOB))
        .await
        .expect("find_job")
        .expect("row");
    assert_eq!(
        row.last_status.as_deref(),
        Some(JobStatus::Success.as_str())
    );
}
