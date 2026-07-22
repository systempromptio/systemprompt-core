//! Error-propagation arms of [`PostgresSessionStore`] driven through a
//! deterministically closed pool: every store operation must surface the
//! repository failure instead of silently degrading.

use rmcp::model::InitializeRequestParams;
use rmcp::transport::streamable_http_server::session::store::{SessionState, SessionStore};
use systemprompt_mcp::middleware::PostgresSessionStore;
use systemprompt_test_fixtures::closed_db_pool;

fn sample_state() -> SessionState {
    let params: InitializeRequestParams = serde_json::from_value(serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "closed-pool-test", "version": "0.0.0" },
    }))
    .expect("valid initialize params");
    SessionState::new(params)
}

#[tokio::test]
async fn load_surfaces_pool_error() {
    let store = PostgresSessionStore::new(&closed_db_pool().await);
    let err = store.load("sess-closed").await.expect_err("closed pool");
    assert!(err.to_string().to_lowercase().contains("pool"));
}

#[tokio::test]
async fn store_surfaces_pool_error() {
    let store = PostgresSessionStore::new(&closed_db_pool().await);
    let err = store
        .store("sess-closed", &sample_state())
        .await
        .expect_err("closed pool");
    assert!(err.to_string().to_lowercase().contains("pool"));
}

#[tokio::test]
async fn delete_surfaces_pool_error() {
    let store = PostgresSessionStore::new(&closed_db_pool().await);
    let err = store.delete("sess-closed").await.expect_err("closed pool");
    assert!(err.to_string().to_lowercase().contains("pool"));
}
