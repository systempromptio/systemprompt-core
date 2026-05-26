//! DB-backed tests for [`McpToolLoader`] / [`ServiceStateService`] constructors
//! and simple read-only methods.

use systemprompt_mcp::orchestration::{McpToolLoader, ServiceStateService};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn loader_new_succeeds() {
    let Some(db) = db().await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpToolLoader::new(&db, registry).expect("ctor");
}

#[tokio::test]
async fn state_service_new_succeeds() {
    let Some(db) = db().await else { return };
    let _ = ServiceStateService::new(&db).expect("ctor");
}

#[tokio::test]
async fn state_service_get_missing_service_returns_none() {
    let Some(db) = db().await else { return };
    let s = ServiceStateService::new(&db).unwrap();
    let r = s
        .get_mcp_service(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn state_service_list_returns_vec() {
    let Some(db) = db().await else { return };
    let s = ServiceStateService::new(&db).unwrap();
    let _ = s.list_mcp_services().await.unwrap();
    let _ = s.list_running_mcp_services().await.unwrap();
}
