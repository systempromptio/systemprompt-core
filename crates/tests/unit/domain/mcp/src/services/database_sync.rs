//! DB-backed tests for the free functions in `services::database::sync`.
//!
//! Each function is invoked against an empty `services` table; no services
//! exist on the per-track DB, so the read-only branches drive line coverage
//! without spawning real processes.

use crate::harness::internal_mcp_config;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::database::sync::{
    cleanup_stale_services, delete_crashed_services, delete_disabled_services, sync_database_state,
};
use systemprompt_test_fixtures::test_db_pool;

#[tokio::test]
async fn cleanup_stale_services_empty_table_returns_ok() {
    let db = test_db_pool().await;
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    cleanup_stale_services(&svc_repo).await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_empty_table_returns_ok() {
    let db = test_db_pool().await;
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    delete_crashed_services(&svc_repo).await.unwrap();
}

#[tokio::test]
async fn sync_database_state_empty_servers_returns_ok() {
    let db = test_db_pool().await;
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    sync_database_state(&svc_repo, &[]).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_removes_only_the_disabled_service() {
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
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
    let deleted = delete_disabled_services(
        &ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ),
        &enabled,
    )
    .await
    .unwrap();
    assert!(deleted >= 1, "at least the disabled service is deleted");
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
