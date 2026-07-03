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
    drop(McpSessionRepository::new(&db).expect("ctor"));
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
async fn cleanup_expired_then_delete_stale_removes_a_seeded_session() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));

    repo.create(&id, None, None).await.unwrap();
    let write = db.write_pool_arc().unwrap();
    sqlx::query(
        "UPDATE mcp_sessions SET expires_at = NOW() - INTERVAL '1 hour', \
         last_activity_at = NOW() - INTERVAL '400 days' WHERE session_id = $1",
    )
    .bind(id.as_str())
    .execute(write.as_ref())
    .await
    .unwrap();

    let expired = repo.cleanup_expired().await.unwrap();
    assert!(
        expired >= 1,
        "cleanup_expired flips at least the seeded past-due session to expired"
    );

    let deleted = repo.delete_stale(365).await.unwrap();
    assert!(
        deleted >= 1,
        "delete_stale removes at least the seeded expired-and-stale session"
    );
    assert!(
        !repo.exists(&id).await.unwrap(),
        "the seeded session is gone after delete_stale"
    );
}

#[tokio::test]
async fn find_initialize_params_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    assert!(repo.find_initialize_params(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn store_find_and_clear_initialize_params_round_trip() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    let params = serde_json::json!({ "protocolVersion": "2025-06-18" });

    repo.store_initialize_params(&id, &params).await.unwrap();
    assert_eq!(
        repo.find_initialize_params(&id).await.unwrap(),
        Some(params)
    );

    repo.clear_initialize_params(&id).await.unwrap();
    assert!(repo.find_initialize_params(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn find_initialize_params_recovers_closed_session() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));
    let params = serde_json::json!({ "protocolVersion": "2025-06-18" });

    repo.store_initialize_params(&id, &params).await.unwrap();
    repo.close(&id).await.unwrap();

    assert_eq!(
        repo.find_initialize_params(&id).await.unwrap(),
        Some(params),
        "a closed session whose init params survive must remain recoverable"
    );

    repo.clear_initialize_params(&id).await.unwrap();
    assert!(
        repo.find_initialize_params(&id).await.unwrap().is_none(),
        "an explicit DELETE clears the recoverable signal"
    );
}

#[tokio::test]
async fn update_activity_reactivates_closed_session() {
    let Some(db) = db().await else { return };
    let repo = McpSessionRepository::new(&db).unwrap();
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));

    repo.create(&id, None, None).await.unwrap();
    repo.close(&id).await.unwrap();
    assert!(repo.find_active(&id).await.unwrap().is_none());

    repo.update_activity(&id).await.unwrap();
    assert!(
        repo.find_active(&id).await.unwrap().is_some(),
        "activity on a restored session must flip its status back to active"
    );
}
