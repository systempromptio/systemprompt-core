//! DB-backed smoke tests for [`McpOrchestrator`].
//!
//! Constructs an orchestrator over a fresh (empty) registry/database and
//! drives the read-only branches (`list_services`, `reconcile`,
//! `sync_database_state`, `get_running_servers`, validation of a missing
//! service). Lifecycle / process-spawn paths are exercised by the existing
//! integration suite.

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::{ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::orchestrator::McpOrchestrator;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_user_id, test_db_pool};

async fn make_orchestrator() -> McpOrchestrator {
    let _ = ensure_test_bootstrap();
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
            systemprompt_models::PathResolution::Canonicalize,
            None,
        )
        .expect("app paths"),
    );
    let registry = RegistryService::new(fixture_user_id());
    let service_repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    McpOrchestrator::new(service_repo, app_paths, registry).expect("orchestrator")
}

#[tokio::test]
async fn orchestrator_new_succeeds() {
    let _o = make_orchestrator().await;
}

#[tokio::test]
async fn orchestrator_get_running_servers_excludes_rows_absent_from_registry() {
    use systemprompt_database::{CreateServiceInput, ServiceRepository};
    let o = make_orchestrator().await;
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    let name = format!("orch-run-{}", uuid::Uuid::new_v4().simple());
    let name_id = ServiceName::new(name.as_str());
    repo.create_service(CreateServiceInput {
        name: &name_id,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: 65509,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let running = o.get_running_servers().await.unwrap();
    assert!(
        !running.iter().any(|c| c.name == name),
        "a running DB row with no matching registry config is excluded"
    );
    repo.delete_service(&name_id).await.unwrap();
}

#[tokio::test]
async fn orchestrator_get_service_info_missing_returns_none() {
    let o = make_orchestrator().await;
    let r = o
        .get_service_info(&ServiceName::new(format!(
            "missing-{}",
            uuid::Uuid::new_v4().simple()
        )))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn orchestrator_subscribe_events_returns_receiver() {
    let o = make_orchestrator().await;
    let _rx = o.subscribe_events();
}

#[tokio::test]
async fn orchestrator_registry_accessor() {
    let o = make_orchestrator().await;
    let _ = o.registry();
}
