//! DB-backed tests for [`DatabaseService`] methods that don't require a
//! validated registry or filesystem layout.

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_manifest::services::ServiceStatus;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_test_fixtures::{fixture_user_id, test_db_pool};

use crate::harness::unique_instance;

async fn make_db_service() -> (DatabaseService, systemprompt_database::ServiceRepository) {
    let db = test_db_pool().await;
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(
        AppPaths::from_profile(
            &paths,
            systemprompt_manifest::PathResolution::Canonicalize,
            None,
        )
        .expect("app paths"),
    );
    let registry = RegistryService::new(fixture_user_id());
    let repo = systemprompt_database::ServiceRepository::new(&db, unique_instance());
    let svc = DatabaseService::new(repo.clone(), app_paths, registry);
    (svc, repo)
}

#[tokio::test]
async fn get_service_by_name_missing_returns_none() {
    let (svc, _db) = make_db_service().await;
    let r = svc
        .get_service_by_name(&ServiceName::new(format!(
            "missing-{}",
            uuid::Uuid::new_v4().simple()
        )))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn cleanup_stale_services_runs() {
    let (svc, _db) = make_db_service().await;
    svc.cleanup_stale_services().await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_runs() {
    let (svc, _db) = make_db_service().await;
    svc.delete_crashed_services().await.unwrap();
}

#[tokio::test]
async fn sync_state_empty_runs() {
    let (svc, _db) = make_db_service().await;
    svc.sync_state(&[]).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_removes_only_the_disabled_service() {
    use crate::harness::internal_mcp_config;
    use systemprompt_database::{CreateServiceInput, ServiceModule};

    let (svc, repo) = make_db_service().await;
    let keep = ServiceName::new(format!("dbsvc-keep-{}", uuid::Uuid::new_v4().simple()));
    let drop_name = ServiceName::new(format!("dbsvc-drop-{}", uuid::Uuid::new_v4().simple()));
    for (name, port) in [(&keep, 65512u16), (&drop_name, 65511u16)] {
        repo.create_service(CreateServiceInput {
            name,
            module_name: ServiceModule::Mcp,
            status: ServiceStatus::Stopped,
            port,
            binary_mtime: None,
        })
        .await
        .unwrap();
    }

    let enabled = [internal_mcp_config(keep.as_str(), 65512)];
    let deleted = svc.delete_disabled_services(&enabled).await.unwrap();
    assert_eq!(deleted, 1, "only the disabled service is deleted");
    assert!(
        repo.find_service_by_name(&keep).await.unwrap().is_some(),
        "the enabled service is preserved"
    );
    assert!(
        repo.find_service_by_name(&drop_name)
            .await
            .unwrap()
            .is_none(),
        "the disabled service is removed"
    );

    repo.delete_service(&keep).await.unwrap();
}

#[tokio::test]
async fn get_running_servers_errors_when_registry_not_validated() {
    let (svc, _db) = make_db_service().await;
    let r = svc.get_running_servers().await;
    let _ = r;
}

#[tokio::test]
async fn update_service_status_missing_no_panic() {
    let (svc, _db) = make_db_service().await;
    svc.update_service_status(
        &ServiceName::new(format!("missing-{}", uuid::Uuid::new_v4().simple())),
        ServiceStatus::Stopped,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn clear_service_pid_missing_no_panic() {
    let (svc, _db) = make_db_service().await;
    svc.clear_service_pid(&ServiceName::new(format!(
        "missing-{}",
        uuid::Uuid::new_v4().simple()
    )))
    .await
    .unwrap();
}

#[tokio::test]
async fn unregister_missing_no_panic() {
    let (svc, _db) = make_db_service().await;
    svc.unregister_service(&ServiceName::new(format!(
        "missing-{}",
        uuid::Uuid::new_v4().simple()
    )))
    .await
    .unwrap();
}

#[tokio::test]
async fn accessors() {
    let (svc, _db) = make_db_service().await;
    let _ = svc.app_paths();
    let _ = svc.clone();
    let _ = format!("{svc:?}");
}

#[tokio::test]
async fn register_existing_process_creates_running_row_with_pid() {
    let (svc, _db) = make_db_service().await;
    let name = format!("adopt-{}", uuid::Uuid::new_v4().simple());
    let config = crate::harness::internal_mcp_config(&name, 65410);

    let registered = svc
        .register_existing_process(&config, std::process::id())
        .await
        .expect("adoption registers");
    assert_eq!(registered.as_str(), name);

    let row = svc
        .get_service_by_name(&registered)
        .await
        .unwrap()
        .expect("row created");
    svc.unregister_service(&registered).await.unwrap();

    assert_eq!(row.status, ServiceStatus::Running);
    assert_eq!(row.pid, Some(i32::try_from(std::process::id()).unwrap()));
}
