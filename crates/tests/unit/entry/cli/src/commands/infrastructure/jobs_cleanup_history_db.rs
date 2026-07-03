//! DB-backed tests for the pool-seamed `infra jobs` cleanup/history commands,
//! driving `execute_with_pool` directly against a fixture pool.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{Duration, Utc};
use serde_json::Value;
use systemprompt_cli::infrastructure::jobs::cleanup_logs::{self, LogCleanupArgs};
use systemprompt_cli::infrastructure::jobs::cleanup_sessions::{self, CleanupSessionsArgs};
use systemprompt_cli::infrastructure::jobs::history::{self, HistoryArgs};
use systemprompt_cli::shared::CommandOutput;
use systemprompt_database::DbPool;
use systemprompt_scheduler::{JobRepository, JobStatus};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn artifact_json(out: &CommandOutput) -> Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&artifact_json(out))
        .unwrap()
        .contains(needle)
}

async fn seed_session(pool: &DbPool, activity: chrono::DateTime<Utc>) -> String {
    let id = format!("sess-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, started_at, last_activity_at) \
         VALUES ($1, NULL, $2, $2)",
    )
    .bind(&id)
    .bind(activity)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    id
}

async fn session_ended(pool: &DbPool, id: &str) -> bool {
    let ended: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT ended_at FROM user_sessions WHERE session_id = $1")
            .bind(id)
            .fetch_one(pool.pool_arc().unwrap().as_ref())
            .await
            .unwrap();
    ended.is_some()
}

#[tokio::test]
async fn cleanup_sessions_dry_run_counts_without_closing() {
    let pool = pool().await;
    let id = seed_session(&pool, Utc::now()).await;

    let out = cleanup_sessions::execute_with_pool(
        CleanupSessionsArgs {
            hours: 1,
            dry_run: true,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(contains(&out, "DRY RUN"));
    assert!(contains(&out, "session_cleanup"));
    assert!(
        !session_ended(&pool, &id).await,
        "dry run must not close the session"
    );
}

#[tokio::test]
async fn cleanup_sessions_closes_inactive_session() {
    let pool = pool().await;
    let id = seed_session(&pool, Utc::now() - Duration::hours(48)).await;

    let out = cleanup_sessions::execute_with_pool(
        CleanupSessionsArgs {
            hours: 1,
            dry_run: false,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(contains(&out, "session_cleanup"));
    assert!(
        session_ended(&pool, &id).await,
        "cleanup must set ended_at on the seeded inactive session"
    );
}

#[tokio::test]
async fn cleanup_logs_dry_run_reports_threshold() {
    let pool = pool().await;

    let out = cleanup_logs::execute_with_pool(
        LogCleanupArgs {
            days: 3650,
            dry_run: true,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(contains(&out, "DRY RUN"));
    assert!(contains(&out, "log_cleanup"));
    assert!(contains(&out, "3650"));
}

#[tokio::test]
async fn cleanup_logs_delete_reports_zero_for_future_threshold() {
    let pool = pool().await;

    let out = cleanup_logs::execute_with_pool(
        LogCleanupArgs {
            days: 3650,
            dry_run: false,
        },
        &pool,
    )
    .await
    .unwrap();

    assert!(contains(&out, "log_cleanup"));
    assert!(contains(&out, "Deleted"));
}

async fn seed_job_run(pool: &DbPool, status: JobStatus, error: Option<&str>) -> String {
    let name = format!("cov-job-{}", Uuid::new_v4().simple());
    let repo = JobRepository::new(pool).unwrap();
    repo.upsert_job(&name, "0 * * * *", true).await.unwrap();
    repo.update_job_execution(&name, status, error, None)
        .await
        .unwrap();
    name
}

#[tokio::test]
async fn history_filters_by_job_name() {
    let pool = pool().await;
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
    let pool = pool().await;
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
    let pool = pool().await;
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
