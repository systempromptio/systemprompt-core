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

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn cleanup_stale_services_empty_table_returns_ok() {
    let Some(db) = db().await else { return };
    cleanup_stale_services(&db).await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_empty_table_returns_ok() {
    let Some(db) = db().await else { return };
    delete_crashed_services(&db).await.unwrap();
}

#[tokio::test]
async fn sync_database_state_empty_servers_returns_ok() {
    let Some(db) = db().await else { return };
    sync_database_state(&db, &[]).await.unwrap();
}

#[tokio::test]
async fn reconcile_running_processes_reports_a_pidless_running_service() {
    let Some(db) = db().await else { return };
    let repo = ServiceRepository::new(&db).unwrap();
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

    let discrepancies = reconcile_running_processes(&db).await.unwrap();
    assert!(
        discrepancies.iter().any(|d| d.contains(&name)),
        "a running service with no live process is reported as a discrepancy"
    );
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn repair_database_inconsistencies_runs() {
    let Some(db) = db().await else { return };
    repair_database_inconsistencies(&db).await.unwrap();
}

#[tokio::test]
async fn delete_disabled_services_removes_only_the_disabled_service() {
    let Some(db) = db().await else { return };
    let repo = ServiceRepository::new(&db).unwrap();
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
    let deleted = delete_disabled_services(&db, &enabled).await.unwrap();
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
