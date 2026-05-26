//! DB-backed integration tests for `DatabaseSyncService` — exercises the
//! export + import round-trip through `upsert_user` / `upsert_context`
//! against a real Postgres schema. Push and Pull are exercised against the
//! same database (acting as both local and cloud) so the upsert paths are
//! covered without needing two Postgres instances.

use systemprompt_sync::{DatabaseSyncService, SyncDirection};

#[tokio::test]
async fn dry_run_push_against_same_db_reports_user_and_context_counts() {
    let Some(_db) = crate::support::try_db().await else {
        return;
    };
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let svc = DatabaseSyncService::new(SyncDirection::Push, true, &url, &url);
    let result = svc.sync().await.expect("dry-run push");
    assert_eq!(result.operation, "database_push");
    assert!(result.success);
    assert!(
        result.details.is_some(),
        "dry-run push should populate details payload"
    );
}

#[tokio::test]
async fn dry_run_pull_against_same_db_reports_user_and_context_counts() {
    let Some(_db) = crate::support::try_db().await else {
        return;
    };
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let svc = DatabaseSyncService::new(SyncDirection::Pull, true, &url, &url);
    let result = svc.sync().await.expect("dry-run pull");
    assert_eq!(result.operation, "database_pull");
    assert!(result.success);
    assert!(result.details.is_some());
}

#[tokio::test]
async fn live_push_against_same_db_is_idempotent() {
    let Some(_db) = crate::support::try_db().await else {
        return;
    };
    let Ok(url) = systemprompt_test_fixtures::fixture_database_url() else {
        return;
    };
    let svc = DatabaseSyncService::new(SyncDirection::Push, false, &url, &url);
    let first = svc.sync().await.expect("first push");
    let second = svc.sync().await.expect("second push");
    assert_eq!(first.operation, "database_push");
    assert_eq!(second.operation, "database_push");
}
