//! DB-backed tests for the free functions in `services::database::sync`.
//!
//! Each test runs against its own instance id, so the instance-wide sweeps see
//! only the rows that test seeded and never another test's.

use crate::harness::internal_mcp_config;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::database::sync::{
    delete_disabled_services, reap_stale_services, sync_database_state,
};
use systemprompt_test_fixtures::{test_db_pool, unique_instance};

#[tokio::test]
async fn reap_stale_services_empty_instance_reaps_nothing() {
    let db = test_db_pool().await;
    let svc_repo = ServiceRepository::new(&db, unique_instance());
    assert_eq!(reap_stale_services(&svc_repo).await.unwrap().total(), 0);
}

#[tokio::test]
async fn reap_stale_services_deletes_error_and_pidless_running_rows() {
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, unique_instance());
    let crashed = ServiceName::new(format!("sync-err-{}", uuid::Uuid::new_v4().simple()));
    let pidless = ServiceName::new(format!("sync-nopid-{}", uuid::Uuid::new_v4().simple()));
    for (name, status, port) in [
        (&crashed, ServiceStatus::Error, 65511u16),
        (&pidless, ServiceStatus::Running, 65510u16),
    ] {
        repo.create_service(CreateServiceInput {
            name,
            module_name: ServiceModule::Mcp,
            status,
            port,
            binary_mtime: None,
        })
        .await
        .unwrap();
    }

    let reaped = reap_stale_services(&repo).await.unwrap();
    assert_eq!(reaped.stale_rows, 2);
    assert!(reaped.dead.is_empty());
    assert!(repo.find_service_by_name(&crashed).await.unwrap().is_none());
    assert!(repo.find_service_by_name(&pidless).await.unwrap().is_none());
}

#[tokio::test]
async fn sync_database_state_empty_servers_returns_ok() {
    let db = test_db_pool().await;
    let svc_repo = ServiceRepository::new(&db, unique_instance());
    sync_database_state(&svc_repo, &[]).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_removes_only_the_disabled_service() {
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, unique_instance());
    let keep = ServiceName::new(format!("sync-keep-{}", uuid::Uuid::new_v4().simple()));
    let drop_name = ServiceName::new(format!("sync-drop-{}", uuid::Uuid::new_v4().simple()));
    for (name, port) in [(&keep, 65514u16), (&drop_name, 65513u16)] {
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

    let enabled = [internal_mcp_config(keep.as_str(), 65514)];
    let deleted = delete_disabled_services(&repo, &enabled).await.unwrap();
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
