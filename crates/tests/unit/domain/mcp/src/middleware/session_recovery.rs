//! DB-backed tests for cross-restart MCP session recovery: the
//! [`PostgresSessionStore`] round-trip and the [`DatabaseSessionHandler`]
//! `restore_session` hook that rmcp drives when a session is missing from
//! memory.

use rmcp::model::InitializeRequestParams;
use rmcp::transport::streamable_http_server::session::store::{SessionState, SessionStore};
use rmcp::transport::streamable_http_server::session::{RestoreOutcome, SessionId, SessionManager};
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_mcp::middleware::{
    DatabaseSessionHandler, PostgresSessionStore, RestorePrincipal, run_as,
};
use systemprompt_test_fixtures::test_db_pool;

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

fn principal(server: &McpServerId) -> RestorePrincipal {
    RestorePrincipal {
        server_id: server.clone(),
        user_id: None,
    }
}

async fn stored_session(
    db: &systemprompt_database::DbPool,
) -> (PostgresSessionStore, SessionId, McpServerId) {
    let repo = std::sync::Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(db));
    let id = random_id();
    let server = McpServerId::new(format!("srv-{}", uuid::Uuid::new_v4().simple()));
    repo.create(
        &systemprompt_identifiers::SessionId::new(id.as_ref()),
        None,
        Some(&server),
    )
    .await
    .unwrap();
    let store = PostgresSessionStore::new(repo);
    store.store(&id, &sample_state()).await.unwrap();
    (store, id, server)
}

#[tokio::test]
async fn session_store_round_trip() {
    let db = test_db_pool().await;
    let (store, id, server) = stored_session(&db).await;

    let loaded = run_as(principal(&server), store.load(&id)).await.unwrap();
    assert!(loaded.is_some());

    store.delete(&id).await.unwrap();
    assert!(
        run_as(principal(&server), store.load(&id))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn session_store_load_unknown_returns_none() {
    let db = test_db_pool().await;
    let store = PostgresSessionStore::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));
    let server = McpServerId::new("srv-unknown");
    assert!(
        run_as(principal(&server), store.load(&random_id()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_session_is_not_restored_outside_a_verified_request() {
    let db = test_db_pool().await;
    let (store, id, _) = stored_session(&db).await;
    assert!(store.load(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn a_session_is_not_restored_for_another_server_or_user() {
    let db = test_db_pool().await;
    let (store, id, server) = stored_session(&db).await;
    let other_server = principal(&McpServerId::new("srv-other"));
    assert!(
        run_as(other_server, store.load(&id))
            .await
            .unwrap()
            .is_none()
    );
    let some_user = RestorePrincipal {
        server_id: server,
        user_id: Some(UserId::new("someone")),
    };
    assert!(run_as(some_user, store.load(&id)).await.unwrap().is_none());
}

#[tokio::test]
async fn restore_session_recreates_local_worker() {
    let db = test_db_pool().await;
    let handler = DatabaseSessionHandler::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));
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
