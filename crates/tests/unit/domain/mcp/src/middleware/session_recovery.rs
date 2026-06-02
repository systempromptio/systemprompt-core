//! DB-backed tests for cross-restart MCP session recovery: the
//! [`PostgresSessionStore`] round-trip and the [`DatabaseSessionHandler`]
//! `restore_session` hook that rmcp drives when a session is missing from
//! memory.

use rmcp::model::InitializeRequestParams;
use rmcp::transport::streamable_http_server::session::store::{SessionState, SessionStore};
use rmcp::transport::streamable_http_server::session::{RestoreOutcome, SessionId, SessionManager};
use systemprompt_mcp::middleware::{DatabaseSessionHandler, PostgresSessionStore};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn sample_state() -> SessionState {
    let params: InitializeRequestParams = serde_json::from_value(serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "session-recovery-test", "version": "0.0.0" },
    }))
    .expect("valid initialize params");
    SessionState::new(params)
}

fn random_id() -> SessionId {
    SessionId::from(format!("sess-{}", uuid::Uuid::new_v4().simple()))
}

#[tokio::test]
async fn session_store_round_trip() {
    let Some(db) = db().await else { return };
    let store = PostgresSessionStore::new(&db);
    let id = random_id();

    store.store(&id, &sample_state()).await.unwrap();
    assert!(store.load(&id).await.unwrap().is_some());

    store.delete(&id).await.unwrap();
    assert!(store.load(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn session_store_load_unknown_returns_none() {
    let Some(db) = db().await else { return };
    let store = PostgresSessionStore::new(&db);
    assert!(store.load(&random_id()).await.unwrap().is_none());
}

#[tokio::test]
async fn restore_session_recreates_local_worker() {
    let Some(db) = db().await else { return };
    let handler = DatabaseSessionHandler::new(&db);
    let id = random_id();

    assert!(!handler.has_session(&id).await.unwrap());

    let outcome = handler
        .restore_session(std::sync::Arc::<str>::clone(&id))
        .await
        .unwrap();
    assert!(matches!(outcome, RestoreOutcome::Restored(_)));
    assert!(handler.has_session(&id).await.unwrap());

    handler.close_session(&id).await.unwrap();
}
