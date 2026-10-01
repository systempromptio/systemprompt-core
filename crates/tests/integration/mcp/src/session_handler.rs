//! `DatabaseSessionHandler` create/has/close lifecycle against a real
//! `DbPool`, exercising the database-persistence branches.

#![cfg(unix)]

use rmcp::transport::streamable_http_server::session::SessionManager;
use systemprompt_mcp::SessionTimeouts;
use systemprompt_mcp::middleware::DatabaseSessionHandler;
use systemprompt_test_fixtures::test_db_pool;

#[tokio::test]
async fn handler_new_succeeds() {
    let db = test_db_pool().await;
    let handler = DatabaseSessionHandler::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));
    let _ = format!("{handler:?}");
}

#[tokio::test]
async fn handler_with_timeouts_succeeds() {
    let db = test_db_pool().await;
    let timeouts = SessionTimeouts {
        init: Some(std::time::Duration::from_secs(5)),
        keep_alive: Some(std::time::Duration::from_secs(30)),
    };
    let handler = DatabaseSessionHandler::with_timeouts(
        std::sync::Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(&db)),
        timeouts,
        None,
    );
    let _ = format!("{handler:?}");
}

#[tokio::test]
async fn create_then_close_session_lifecycle() {
    let db = test_db_pool().await;
    let handler = DatabaseSessionHandler::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));

    let (session_id, _transport) = handler
        .create_session()
        .await
        .expect("create_session succeeds");

    assert!(
        handler.has_session(&session_id).await.unwrap_or(false),
        "session must be visible immediately after create"
    );

    handler
        .close_session(&session_id)
        .await
        .expect("close_session succeeds");

    assert!(
        !handler.has_session(&session_id).await.unwrap_or(true),
        "session must not be visible after close"
    );
}

#[tokio::test]
async fn close_unknown_session_is_idempotent() {
    let db = test_db_pool().await;
    let handler = DatabaseSessionHandler::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));

    let fake: rmcp::transport::streamable_http_server::session::SessionId =
        format!("nonexistent-{}", uuid::Uuid::new_v4()).into();
    handler.close_session(&fake).await.expect("idempotent");
}

#[tokio::test]
async fn has_session_returns_false_for_unknown() {
    let db = test_db_pool().await;
    let handler = DatabaseSessionHandler::new(std::sync::Arc::new(
        systemprompt_mcp::repository::McpSessionRepository::new(&db),
    ));

    let fake: rmcp::transport::streamable_http_server::session::SessionId =
        format!("nonexistent-{}", uuid::Uuid::new_v4()).into();
    assert!(!handler.has_session(&fake).await.unwrap_or(true));
}

#[tokio::test]
async fn mcp_state_exposes_db_pool() {
    use systemprompt_mcp::McpState;
    let db = test_db_pool().await;
    let state = McpState::new(db.clone());
    let _ = state.db_pool();
    let _ = format!("{state:?}");
    let cloned = state.clone();
    let _ = cloned.db_pool();
}
