//! DB-backed tests for `SchedulerRepository` / `JobRepository` — exercises
//! the `scheduled_jobs` upsert, lookup, status-update, and run-count paths
//! against a real Postgres schema.

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_scheduler::{JobRunRecord, JobStatus, SchedulerRepository};
use systemprompt_test_fixtures::test_db_pool;

fn unique_job_name() -> String {
    format!("test_job_{}", uuid::Uuid::new_v4().simple())
}

async fn cleanup_job(pool: &DbPool, job_name: &str) {
    let write = pool.write_pool();
    let _ = sqlx::query("DELETE FROM scheduled_jobs WHERE job_name = $1")
        .bind(job_name)
        .execute(&*write)
        .await;
}

#[tokio::test]
async fn upsert_job_inserts_then_updates_in_place() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let name = unique_job_name();

    repo.upsert_job(&name, "0 * * * *", true)
        .await
        .expect("first upsert");
    let inserted = repo
        .find_job(&name)
        .await
        .expect("find")
        .expect("row present");
    assert_eq!(inserted.schedule, "0 * * * *");
    assert!(inserted.enabled);

    repo.upsert_job(&name, "*/5 * * * *", false)
        .await
        .expect("second upsert");
    let updated = repo.find_job(&name).await.expect("find").expect("row");
    assert_eq!(updated.schedule, "*/5 * * * *");
    assert!(!updated.enabled);

    cleanup_job(&pool, &name).await;
}

#[tokio::test]
async fn find_job_returns_none_for_unknown_name() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let nope = format!("no_such_job_{}", uuid::Uuid::new_v4().simple());
    let result = repo.find_job(&nope).await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn update_job_execution_records_status_and_error() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let name = unique_job_name();
    repo.upsert_job(&name, "0 0 * * *", true)
        .await
        .expect("upsert");

    let next_run = Utc::now() + chrono::Duration::hours(1);
    repo.update_job_execution(
        &name,
        JobRunRecord {
            status: JobStatus::Failed,
            error: Some("boom"),
            message: None,
            next_run: Some(next_run),
            instance_id: &InstanceId::new("test-node"),
        },
    )
    .await
    .expect("update execution");

    let row = repo.find_job(&name).await.expect("find").expect("row");
    assert_eq!(row.last_status.as_deref(), Some("failed"));
    assert_eq!(row.last_error.as_deref(), Some("boom"));
    assert!(row.last_run.is_some());
    assert!(row.next_run.is_some());

    cleanup_job(&pool, &name).await;
}

#[tokio::test]
async fn recorded_runs_advance_run_count() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let name = unique_job_name();
    repo.upsert_job(&name, "0 0 * * *", true)
        .await
        .expect("upsert");
    let before = repo
        .find_job(&name)
        .await
        .expect("find")
        .expect("row")
        .run_count;
    for _ in 0..2 {
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                message: None,
                next_run: None,
                instance_id: &InstanceId::new("test-node"),
            },
        )
        .await
        .expect("record run");
    }
    let after = repo
        .find_job(&name)
        .await
        .expect("find")
        .expect("row")
        .run_count;
    assert_eq!(after, before + 2);

    cleanup_job(&pool, &name).await;
}

#[tokio::test]
async fn list_enabled_jobs_contains_inserted_enabled_job() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let enabled_name = unique_job_name();
    let disabled_name = unique_job_name();
    repo.upsert_job(&enabled_name, "0 0 * * *", true)
        .await
        .expect("upsert enabled");
    repo.upsert_job(&disabled_name, "0 0 * * *", false)
        .await
        .expect("upsert disabled");

    let list = repo.list_enabled_jobs().await.expect("list");
    assert!(list.iter().any(|j| j.job_name == enabled_name));
    assert!(!list.iter().any(|j| j.job_name == disabled_name));

    cleanup_job(&pool, &enabled_name).await;
    cleanup_job(&pool, &disabled_name).await;
}

#[tokio::test]
async fn cleanup_empty_contexts_runs_without_error() {
    let pool = test_db_pool().await;
    let repo = SchedulerRepository::new(&pool);
    let deleted = repo.cleanup_empty_contexts(24).await.expect("cleanup runs");
    let _ = deleted;
}
