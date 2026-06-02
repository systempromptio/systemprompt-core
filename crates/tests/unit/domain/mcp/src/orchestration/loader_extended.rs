// Additional McpToolLoader paths beyond the constructor coverage in
// loader_state.rs: the empty-server-names early return in create_mcp_extensions
// (does not touch global Config) and the missing-service retry/backoff loop in
// load_server_tools (returns the "not found" error after exhausting retries).
// All DB-backed and skip-guarded; no live MCP server is contacted.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::orchestration::McpToolLoader;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-loader"),
        TraceId::new("t-loader"),
        ContextId::generate(),
        AgentName::new("agent-loader"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "user-loader",
    )))
}

#[tokio::test]
async fn create_mcp_extensions_empty_returns_empty_vec() {
    let Some(db) = db().await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let loader = McpToolLoader::new(&db, registry).expect("ctor");

    // The empty-slice fast path returns before consulting global Config.
    let result = loader
        .create_mcp_extensions(&[], "http://localhost:8080", &ctx())
        .await
        .expect("empty slice is the early-return branch");
    assert!(result.is_empty());
}

#[tokio::test]
async fn load_server_tools_missing_service_errors_after_retries() {
    let Some(db) = db().await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let loader = McpToolLoader::new(&db, registry).expect("ctor");

    let missing = format!("missing-{}", uuid::Uuid::new_v4().simple());
    let result = loader.load_server_tools(&missing, &ctx()).await;

    // No services row exists, so after exhausting the DB-lag retries the loader
    // surfaces a "not found in services database" error.
    let err = result.expect_err("missing service must error");
    let msg = err.to_string();
    assert!(
        msg.contains("not found") || msg.contains(&missing),
        "unexpected: {msg}"
    );
}

#[tokio::test]
async fn service_manager_accessor_returns_reference() {
    let Some(db) = db().await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let loader = McpToolLoader::new(&db, registry).expect("ctor");

    let sm = loader.service_manager();
    // Drive a read-only method through the borrowed accessor.
    let missing = format!("none-{}", uuid::Uuid::new_v4().simple());
    let found = sm.get_mcp_service(&missing).await.expect("query ok");
    assert!(found.is_none());
}
