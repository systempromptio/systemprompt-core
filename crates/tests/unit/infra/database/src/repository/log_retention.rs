//! DB-backed tests for the `LoggingRepository` retention sweeps.
//!
//! They live in this crate rather than beside the other logging tests
//! because the orphan sweep is table-wide: a sibling test that inserts a log
//! row under a synthetic user id would lose it mid-run.
//!
//! Each test seeds uniquely-keyed rows and asserts only on those rows — the
//! cleanup DELETEs are table-wide, so returned counts are checked as lower
//! bounds and the definitive assertion is that the seeded row is gone.

use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_logging::LoggingRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn repo_and_pool_or_skip() -> Option<(LoggingRepository, sqlx::PgPool)> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let pg = db.write_pool();
    Some((LoggingRepository::new(&db).ok()?, (*pg).clone()))
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn insert_log(pool: &sqlx::PgPool, id: &str, user_id: Option<&str>, age_days: i64) {
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id) VALUES ($1, $2, \
         'INFO', 'cleanup-test', 'cleanup fixture', $3)",
    )
    .bind(id)
    .bind(Utc::now() - Duration::days(age_days))
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert log fixture");
}

async fn log_exists(pool: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM logs WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("log existence probe")
}

#[tokio::test]
async fn delete_old_logs_removes_rows_past_cutoff_and_keeps_recent() {
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
        return;
    };
    // A `logs` row must never carry a NULL user_id: every global read of the
    // table decodes that column as non-Option, so one leaked NULL row breaks
    // unrelated suites (`infra logs export`, the logging maintenance service).
    // A real user also keeps the fresh row out of the table-wide orphan sweep
    // the sibling test runs.
    let owner = unique("cleanup_log_owner");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&owner)
        .bind(format!("{owner}@cleanup.test"))
        .execute(&pg)
        .await
        .expect("insert log owner fixture");

    let old_id = unique("old_log");
    let fresh_id = unique("fresh_log");
    insert_log(&pg, &old_id, Some(&owner), 4000).await;
    insert_log(&pg, &fresh_id, Some(&owner), 0).await;

    let cutoff = Utc::now() - Duration::days(3650);
    let counted = repo.count_logs_before(cutoff).await.expect("count old");
    assert!(counted >= 1);

    let deleted = repo.cleanup_old_logs(cutoff).await.expect("delete old");
    assert!(deleted >= 1);

    assert!(!log_exists(&pg, &old_id).await);
    assert!(log_exists(&pg, &fresh_id).await);

    let _ = sqlx::query("DELETE FROM logs WHERE id = $1")
        .bind(&fresh_id)
        .execute(&pg)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&owner)
        .execute(&pg)
        .await;
}

// One test covers count + delete: the orphan sweep is table-wide, so two
// tests each seeding an orphan race each other's DELETE.
#[tokio::test]
async fn orphaned_logs_are_counted_then_removed_for_missing_users() {
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
        return;
    };
    let orphan_id = unique("orphan_log");
    let ghost_user = unique("ghost_user");
    insert_log(&pg, &orphan_id, Some(&ghost_user), 0).await;

    let ghost = UserId::new(&ghost_user);
    let seen = repo.distinct_log_user_ids().await.expect("log owners");
    assert!(
        seen.contains(&ghost),
        "the ghost owner is reported for the users domain to check"
    );

    let count = repo
        .count_logs_for_users(std::slice::from_ref(&ghost))
        .await
        .expect("count orphaned");
    assert!(count >= 1);
    assert!(
        log_exists(&pg, &orphan_id).await,
        "counting must not delete"
    );

    let deleted = repo
        .delete_logs_for_users(std::slice::from_ref(&ghost))
        .await
        .expect("delete orphaned");
    assert!(deleted >= 1);
    assert!(!log_exists(&pg, &orphan_id).await);
}
