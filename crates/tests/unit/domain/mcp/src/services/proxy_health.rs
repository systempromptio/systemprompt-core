//! DB-backed tests for [`ProxyHealthCheck`].

use systemprompt_mcp::services::monitoring::proxy_health::{ProxyHealthCheck, RoutableService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn proxy_health_new_succeeds() {
    let Some(db) = db().await else { return };
    let _ = ProxyHealthCheck::new(&db).expect("ctor");
}

#[tokio::test]
async fn can_route_traffic_missing_service_returns_false() {
    let Some(db) = db().await else { return };
    let p = ProxyHealthCheck::new(&db).unwrap();
    let r = p
        .can_route_traffic(&format!("missing-{}", uuid::Uuid::new_v4().simple()), 65530)
        .await
        .unwrap();
    assert!(!r);
}

#[tokio::test]
async fn list_routable_services_returns_vec() {
    let Some(db) = db().await else { return };
    let p = ProxyHealthCheck::new(&db).unwrap();
    let r = p.list_routable_services().await.unwrap();
    let _ = r.len();
}

#[tokio::test]
async fn can_route_traffic_running_service_unreachable_port_returns_false() {
    use systemprompt_database::{CreateServiceInput, ServiceRepository};
    let Some(db) = db().await else { return };
    let p = ProxyHealthCheck::new(&db).unwrap();
    let repo = ServiceRepository::new(&db).unwrap();
    let name = format!("ph-run-{}", uuid::Uuid::new_v4().simple());
    let port = 65519;
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    let r = p.can_route_traffic(&name, port).await.unwrap();
    assert!(!r);
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn can_route_traffic_stopped_service_returns_false() {
    use systemprompt_database::{CreateServiceInput, ServiceRepository};
    let Some(db) = db().await else { return };
    let p = ProxyHealthCheck::new(&db).unwrap();
    let repo = ServiceRepository::new(&db).unwrap();
    let name = format!("ph-stop-{}", uuid::Uuid::new_v4().simple());
    let port = 65518;
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "stopped",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    let r = p.can_route_traffic(&name, port).await.unwrap();
    assert!(!r);
    repo.delete_service(&name).await.unwrap();
}

#[test]
fn routable_service_value_type() {
    let s = RoutableService {
        name: "n".to_owned(),
        port: 1,
        pid: Some(123),
        health: "healthy".to_owned(),
    };
    let _ = s.clone();
    let _ = format!("{s:?}");
    assert_eq!(s.name, "n");
}
