//! DB-backed tests for the free functions in `services::database::sync`.
//!
//! Each function is invoked against an empty `services` table; no services
//! exist on the per-track DB, so the read-only branches drive line coverage
//! without spawning real processes.

use systemprompt_mcp::services::database::sync::{
    cleanup_stale_services, delete_crashed_services, delete_disabled_services,
    reconcile_running_processes, repair_database_inconsistencies, sync_database_state,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn cleanup_stale_services_empty_table_returns_ok() {
    let Some(db) = db().await else { return };
    cleanup_stale_services(&db).await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_empty_table_returns_ok() {
    let Some(db) = db().await else { return };
    delete_crashed_services(&db).await.unwrap();
}

#[tokio::test]
async fn sync_database_state_empty_servers_returns_ok() {
    let Some(db) = db().await else { return };
    sync_database_state(&db, &[]).await.unwrap();
}

#[tokio::test]
async fn reconcile_running_processes_returns_vec() {
    let Some(db) = db().await else { return };
    let _ = reconcile_running_processes(&db).await.unwrap();
}

#[tokio::test]
async fn repair_database_inconsistencies_runs() {
    let Some(db) = db().await else { return };
    repair_database_inconsistencies(&db).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_empty_runs() {
    let Some(db) = db().await else { return };
    let _ = delete_disabled_services(&db, &[]).await.unwrap();
}
