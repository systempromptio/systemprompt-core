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
    drop(McpToolLoader::new(&db, registry).expect("ctor"));
}

#[tokio::test]
async fn state_service_new_succeeds() {
    let Some(db) = db().await else { return };
    drop(ServiceStateService::new(&db).expect("ctor"));
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
async fn state_service_list_surfaces_seeded_service_and_filters_by_status() {
    use systemprompt_database::{CreateServiceInput, ServiceRepository};
    let Some(db) = db().await else { return };
    let s = ServiceStateService::new(&db).unwrap();
    let repo = ServiceRepository::new(&db).unwrap();

    let running = format!("ls-run-{}", uuid::Uuid::new_v4().simple());
    let stopped = format!("ls-stop-{}", uuid::Uuid::new_v4().simple());
    repo.create_service(CreateServiceInput {
        name: &running,
        module_name: "mcp",
        status: "running",
        port: 65517,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo.create_service(CreateServiceInput {
        name: &stopped,
        module_name: "mcp",
        status: "stopped",
        port: 65516,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let all = s.list_mcp_services().await.unwrap();
    let running_row = all
        .iter()
        .find(|r| r.name == running)
        .expect("seeded running service surfaces in list_mcp_services");
    assert_eq!(running_row.status, "running");
    assert_eq!(running_row.port, 65517);
    assert!(
        all.iter().any(|r| r.name == stopped),
        "stopped service also surfaces in the unfiltered list"
    );

    let running_only = s.list_running_mcp_services().await.unwrap();
    assert!(
        running_only.iter().any(|r| r.name == running),
        "running service surfaces in list_running_mcp_services"
    );
    assert!(
        !running_only.iter().any(|r| r.name == stopped),
        "stopped service is filtered out of list_running_mcp_services"
    );

    repo.delete_service(&running).await.unwrap();
    repo.delete_service(&stopped).await.unwrap();
}
