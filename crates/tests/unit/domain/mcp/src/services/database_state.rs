//! DB-backed tests for `services::database::state` helpers.

use std::path::PathBuf;
use systemprompt_mcp::services::database::state::{
    get_binary_mtime, get_service_by_name, unregister_service,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[test]
fn get_binary_mtime_missing_file_returns_none() {
    let path = PathBuf::from(format!(
        "/tmp/no-such-{}",
        uuid::Uuid::new_v4().simple()
    ));
    assert!(get_binary_mtime(&path).is_none());
}

#[test]
fn get_binary_mtime_existing_file_returns_some() {
    let path = std::env::current_exe().expect("current exe");
    assert!(get_binary_mtime(&path).is_some());
}

#[tokio::test]
async fn get_service_by_name_missing_returns_none() {
    let Some(db) = db().await else { return };
    let r = get_service_by_name(&db, &format!("svc-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn unregister_service_missing_no_panic() {
    let Some(db) = db().await else { return };
    unregister_service(&db, &format!("svc-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
}
