//! DB-backed tests for the free functions in `services::database::sync`.
//!
//! Each function is invoked against an empty `services` table; no services
//! exist on the per-track DB, so the read-only branches drive line coverage
//! without spawning real processes.

use crate::harness::internal_mcp_config;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::services::database::sync::{
    cleanup_stale_services, delete_crashed_services, delete_disabled_services,
    reconcile_running_processes, repair_database_inconsistencies, sync_database_state,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn cleanup_stale_services_empty_table_returns_ok() {
    let Some(db) = db_or_skip().await else { return };
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    cleanup_stale_services(&svc_repo).await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_empty_table_returns_ok() {
    let Some(db) = db_or_skip().await else { return };
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    delete_crashed_services(&svc_repo).await.unwrap();
}

#[tokio::test]
async fn sync_database_state_empty_servers_returns_ok() {
    let Some(db) = db_or_skip().await else { return };
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    sync_database_state(&svc_repo, &[]).await.unwrap();
}

#[tokio::test]
async fn reconcile_running_processes_reports_a_pidless_running_service() {
    let Some(db) = db_or_skip().await else { return };
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    let name = format!("sync-rec-{}", uuid::Uuid::new_v4().simple());
    let port = 65515;
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let discrepancies = reconcile_running_processes(&svc_repo).await.unwrap();
    assert!(
        discrepancies.iter().any(|d| d.contains(&name)),
        "a running service with no live process is reported as a discrepancy"
    );
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn repair_database_inconsistencies_runs() {
    let Some(db) = db_or_skip().await else { return };
    let svc_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    repair_database_inconsistencies(&svc_repo).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_removes_only_the_disabled_service() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    )
    .unwrap();
    let keep = format!("sync-keep-{}", uuid::Uuid::new_v4().simple());
    let drop_name = format!("sync-drop-{}", uuid::Uuid::new_v4().simple());
    for (name, port) in [(&keep, 65514u16), (&drop_name, 65513u16)] {
        repo.create_service(CreateServiceInput {
            name,
            module_name: "mcp",
            status: "stopped",
            port,
            binary_mtime: None,
        })
        .await
        .unwrap();
    }

    let enabled = [internal_mcp_config(&keep, 65514)];
    let deleted = delete_disabled_services(
        &ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        )
        .unwrap(),
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
