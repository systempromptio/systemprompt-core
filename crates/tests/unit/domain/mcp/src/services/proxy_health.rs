//! DB-backed tests for [`ProxyHealthCheck`].

use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::monitoring::proxy_health::{ProxyHealthCheck, RoutableService};
use systemprompt_test_fixtures::{test_db_pool, unique_instance};

#[tokio::test]
async fn can_route_traffic_missing_service_returns_false() {
    let db = test_db_pool().await;
    let p = ProxyHealthCheck::new(systemprompt_database::ServiceRepository::new(
        &db,
        unique_instance(),
    ));
    let r = p
        .can_route_traffic(
            &ServiceName::new(format!("missing-{}", uuid::Uuid::new_v4().simple())),
            65530,
        )
        .await
        .unwrap();
    assert!(!r);
}

#[tokio::test]
async fn list_routable_services_excludes_service_with_unresponsive_port() {
    use systemprompt_database::{
        CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus,
    };
    use systemprompt_identifiers::ServiceName;
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, unique_instance());
    let p = ProxyHealthCheck::new(repo.clone());
    let name = format!("ph-list-{}", uuid::Uuid::new_v4().simple());
    let name_id = ServiceName::new(name.as_str());
    let port = 65510;
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let routable = p.list_routable_services().await.unwrap();
    assert!(
        !routable.iter().any(|s| s.name == name_id),
        "a running service on an unresponsive port is not routable"
    );
    let after = repo.find_service_by_name(&name_id).await.unwrap().unwrap();
    assert_eq!(
        after.status,
        ServiceStatus::Stopped,
        "an unroutable running service is marked stopped"
    );
    repo.delete_service(&name_id).await.unwrap();
}

#[tokio::test]
async fn can_route_traffic_running_service_unreachable_port_returns_false() {
    use systemprompt_database::{
        CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus,
    };
    use systemprompt_identifiers::ServiceName;
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, unique_instance());
    let p = ProxyHealthCheck::new(repo.clone());
    let name = format!("ph-run-{}", uuid::Uuid::new_v4().simple());
    let name_id = ServiceName::new(name.as_str());
    let port = 65519;
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    let r = p.can_route_traffic(&name_id, port).await.unwrap();
    assert!(!r);
    repo.delete_service(&name_id).await.unwrap();
}

#[tokio::test]
async fn can_route_traffic_stopped_service_returns_false() {
    use systemprompt_database::{
        CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus,
    };
    use systemprompt_identifiers::ServiceName;
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, unique_instance());
    let p = ProxyHealthCheck::new(repo.clone());
    let name = format!("ph-stop-{}", uuid::Uuid::new_v4().simple());
    let name_id = ServiceName::new(name.as_str());
    let port = 65518;
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Stopped,
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    let r = p.can_route_traffic(&name_id, port).await.unwrap();
    assert!(!r);
    repo.delete_service(&name_id).await.unwrap();
}

#[test]
fn routable_service_value_type() {
    let s = RoutableService {
        name: ServiceName::new("n"),
        port: 1,
        pid: Some(123),
        health: "healthy".to_owned(),
    };
    let _ = s.clone();
    let _ = format!("{s:?}");
    assert_eq!(s.name, "n");
}
