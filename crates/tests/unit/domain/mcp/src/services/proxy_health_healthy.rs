//! Healthy-path tests for [`ProxyHealthCheck`]: a running services row whose
//! port answers the scripted MCP handshake routes traffic, appears in the
//! routable list, and a responsive-but-non-MCP port is downgraded to `error`.

use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::services::monitoring::proxy_health::ProxyHealthCheck;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use wiremock::MockServer;

use crate::harness::{default_tools_json, mount_mcp_endpoint};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn can_route_traffic_true_for_running_service_with_live_mcp_endpoint() {
    let Some(db) = db().await else { return };
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let port = mock.address().port();

    let repo = ServiceRepository::new(&db).unwrap();
    let name = format!("ph-live-{}", uuid::Uuid::new_v4().simple());
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let p = ProxyHealthCheck::new(&db).unwrap();
    let routable = p.can_route_traffic(&name, port).await.unwrap();
    let status = repo
        .find_service_by_name(&name)
        .await
        .unwrap()
        .unwrap()
        .status;
    repo.delete_service(&name).await.unwrap();

    assert!(routable, "a live MCP endpoint routes traffic");
    assert_eq!(status, "running", "a routable service keeps its status");
}

#[tokio::test]
async fn can_route_traffic_responsive_non_mcp_port_marks_service_error() {
    let Some(db) = db().await else { return };
    let mock = MockServer::start().await;
    let port = mock.address().port();

    let repo = ServiceRepository::new(&db).unwrap();
    let name = format!("ph-err-{}", uuid::Uuid::new_v4().simple());
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let p = ProxyHealthCheck::new(&db).unwrap();
    let routable = p.can_route_traffic(&name, port).await.unwrap();
    let status = repo
        .find_service_by_name(&name)
        .await
        .unwrap()
        .unwrap()
        .status;
    repo.delete_service(&name).await.unwrap();

    assert!(
        !routable,
        "a TCP-only port with no MCP handshake is unroutable"
    );
    assert_eq!(status, "error", "MCP connect failure downgrades to error");
}

#[tokio::test]
async fn list_routable_services_includes_service_with_responsive_port() {
    let Some(db) = db().await else { return };
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let port = mock.address().port();

    let repo = ServiceRepository::new(&db).unwrap();
    let name = format!("ph-ok-{}", uuid::Uuid::new_v4().simple());
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let p = ProxyHealthCheck::new(&db).unwrap();
    let routable = p.list_routable_services().await.unwrap();
    repo.delete_service(&name).await.unwrap();

    let entry = routable
        .iter()
        .find(|s| s.name == name)
        .expect("responsive service is listed as routable");
    assert_eq!(entry.port, port);
    assert_eq!(entry.health, "healthy");
}
