//! DB-backed tests for the pool-seamed `infra jobs history` command, driving
//! `execute_with_pool` directly against a fixture pool.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::Value;
use systemprompt_cli::infrastructure::jobs::history::{self, HistoryArgs};
use systemprompt_cli::shared::CommandOutput;
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_scheduler::{JobRepository, JobRunRecord, JobStatus};
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;


fn artifact_json(out: &CommandOutput) -> Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&artifact_json(out))
        .unwrap()
        .contains(needle)
}

async fn seed_job_run(pool: &DbPool, status: JobStatus, error: Option<&str>) -> String {
    let name = format!("cov-job-{}", Uuid::new_v4().simple());
    let repo = JobRepository::new(pool).unwrap();
    repo.upsert_job(&name, "0 * * * *", true).await.unwrap();
    repo.update_job_execution(
        &name,
        JobRunRecord {
            status: status,
            error: error,
            message: None,
            next_run: None,
            instance_id: &InstanceId::new("test-node"),
        },
    )
    .await
    .unwrap();
    name
}

#[tokio::test]
async fn history_filters_by_job_name() {
    let pool = test_db_pool().await;
    let name = seed_job_run(&pool, JobStatus::Success, None).await;

    let out = history::execute_with_pool(
        HistoryArgs {
            job: Some(name.clone()),
            limit: 20,
            status: None,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(contains(&out, &name));
    assert!(contains(&out, "success"));
}

#[tokio::test]
async fn history_missing_job_yields_empty() {
    let pool = test_db_pool().await;
    let ghost = format!("no-such-{}", Uuid::new_v4().simple());

    let out = history::execute_with_pool(
        HistoryArgs {
            job: Some(ghost.clone()),
            limit: 20,
            status: None,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(!contains(&out, &ghost));
}

#[tokio::test]
async fn history_status_filter_excludes_mismatch() {
    let pool = test_db_pool().await;
    let failed = seed_job_run(&pool, JobStatus::Failed, Some("boom")).await;

    let listed = history::execute_with_pool(
        HistoryArgs {
            job: None,
            limit: 500,
            status: Some("failed".to_owned()),
        },
        &pool,
    )
    .await
    .unwrap();
    assert!(contains(&listed, &failed));

    let filtered = history::execute_with_pool(
        HistoryArgs {
            job: None,
            limit: 500,
            status: Some("success".to_owned()),
        },
        &pool,
    )
    .await
    .unwrap();
    assert!(!contains(&filtered, &failed));
}
