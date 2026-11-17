//! The retention job's archived-user pass through the public job entry: a
//! purge that leaves a member's rows uncovered (a recorded table owner that
//! is not admitted and has no recorded declarations) fails the run and
//! reports the count; a complete purge succeeds. Each case owns a
//! disposable database, so the ownership rows it records reach no other test.

use systemprompt_scheduler::DatabaseCleanupJob;
use systemprompt_test_fixtures::{DisposableDb, fixture_actor};
use systemprompt_traits::{Job, JobContext, JobResult};

async fn archive_a_user(pool: &sqlx::PgPool, tag: &str) {
    sqlx::query(
        "INSERT INTO users (id, name, email, status, archived_at) \
         VALUES ($1, $1, $1 || '@example.test', 'deleted', NOW() - make_interval(days => 4000))",
    )
    .bind(format!("archived-{tag}"))
    .execute(pool)
    .await
    .expect("seed an archived user");
}

async fn run(database: &DisposableDb) -> JobResult {
    let pool = database.test_pool().await;
    let dependencies = crate::test_extension::job_dependencies(&pool, database.url()).await;
    DatabaseCleanupJob
        .execute(&JobContext::new(fixture_actor(), dependencies).with_enforce(true))
        .await
        .expect("the retention job runs")
}

#[tokio::test]
async fn an_archived_user_purge_left_uncovered_fails_the_run_with_its_count() {
    let database = DisposableDb::with_schema("sched_archived_uncovered").await;
    let raw = database.test_pool().await.pool();
    archive_a_user(&raw, "uncovered").await;
    sqlx::query(
        "INSERT INTO schema_table_owners (table_name, extension_id, standing) \
         VALUES ('public.ghost_rows', 'ghost_member', 'install')",
    )
    .execute(raw.as_ref())
    .await
    .expect("record a table owner the composition does not admit");

    let result = run(&database).await;

    assert!(!result.success, "{result:?}");
    let message = result.message.unwrap_or_default();
    assert!(message.contains("incomplete_purges=1"), "{message}");
    database.drop_now().await;
}

#[tokio::test]
async fn a_complete_archived_user_purge_succeeds() {
    let database = DisposableDb::with_schema("sched_archived_complete").await;
    let raw = database.test_pool().await.pool();
    archive_a_user(&raw, "complete").await;

    let result = run(&database).await;

    assert!(result.success, "{result:?}");
    let message = result.message.unwrap_or_default();
    assert!(
        message.contains("archived_users=1 incomplete_purges=0"),
        "{message}"
    );
    database.drop_now().await;
}
