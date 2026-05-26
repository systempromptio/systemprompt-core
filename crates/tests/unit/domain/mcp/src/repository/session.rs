//! DB-backed tests for [`McpSessionRepository`].

use systemprompt_identifiers::SessionId;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = db().await else { return };
    let _ = McpSessionRepository::new(&db).expect("ctor");
}

#[tokio::test]
async fn exists_for_random_returns_false() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    assert!(!repo.exists(&id).await.unwrap());
}

#[tokio::test]
async fn find_active_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    assert!(repo.find_active(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn update_close_on_missing_session_no_panic() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    repo.update_last_event_id(&id, "evt").await.unwrap();
    repo.update_activity(&id).await.unwrap();
    repo.close(&id).await.unwrap();
}

#[tokio::test]
async fn cleanup_and_delete_stale_execute() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let _ = repo.cleanup_expired().await.unwrap();
    let _ = repo.delete_stale(365).await.unwrap();
}
