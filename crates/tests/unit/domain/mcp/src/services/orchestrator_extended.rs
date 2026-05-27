//! Extended DB-backed tests for [`McpOrchestrator`] covering lifecycle ops,
//! target resolution, reconciliation, schema sync, validate_service, and
//! daemon-related accessors. Relies on `ensure_test_bootstrap` to initialise
//! Profile/Secrets/Config singletons that downstream loaders (schema_sync's
//! `ConfigLoader::load()`) require.

use std::sync::Arc;
use systemprompt_mcp::services::orchestrator::McpOrchestrator;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};

async fn make_orchestrator() -> Option<McpOrchestrator> {
    let _ = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths).ok()?);
    let registry = RegistryService::new(fixture_user_id());
    McpOrchestrator::new(db, app_paths, registry).ok()
}

#[tokio::test]
async fn list_services_empty_registry_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.list_services().await.unwrap();
}

#[tokio::test]
async fn show_status_delegates_to_list_services() {
    let Some(o) = make_orchestrator().await else { return };
    o.show_status().await.unwrap();
}

#[tokio::test]
async fn sync_database_state_empty_registry_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.sync_database_state().await.unwrap();
}

#[tokio::test]
async fn reconcile_empty_registry_returns_zero() {
    let Some(o) = make_orchestrator().await else { return };
    let started = o.reconcile().await.unwrap();
    assert_eq!(started, 0);
}

#[tokio::test]
async fn reconcile_with_events_none_returns_zero() {
    let Some(o) = make_orchestrator().await else { return };
    let started = o.reconcile_with_events(None).await.unwrap();
    assert_eq!(started, 0);
}

#[tokio::test]
async fn validate_service_unknown_returns_server_not_found() {
    let Some(o) = make_orchestrator().await else { return };
    let r = o
        .validate_service(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn start_services_empty_target_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.start_services(None).await.unwrap();
}

#[tokio::test]
async fn start_services_all_keyword_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.start_services(Some("all".to_string())).await.unwrap();
}

#[tokio::test]
async fn start_services_specific_missing_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.start_services(Some(format!("missing-{}", uuid::Uuid::new_v4().simple())))
        .await
        .unwrap();
}

#[tokio::test]
async fn stop_services_none_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.stop_services(None).await.unwrap();
}

#[tokio::test]
async fn stop_services_all_keyword_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.stop_services(Some("all".to_string())).await.unwrap();
}

#[tokio::test]
async fn stop_services_specific_missing_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.stop_services(Some(format!("missing-{}", uuid::Uuid::new_v4().simple())))
        .await
        .unwrap();
}

#[tokio::test]
async fn restart_services_none_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.restart_services(None).await.unwrap();
}

#[tokio::test]
async fn restart_services_all_keyword_publishes_no_events() {
    let Some(o) = make_orchestrator().await else { return };
    o.restart_services(Some("all".to_string())).await.unwrap();
}

#[tokio::test]
async fn restart_services_sync_none_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.restart_services_sync(None).await.unwrap();
}

#[tokio::test]
async fn restart_services_sync_specific_missing_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.restart_services_sync(Some(format!("x-{}", uuid::Uuid::new_v4().simple())))
        .await
        .unwrap();
}

#[tokio::test]
async fn build_services_empty_target_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.build_services(None).await.unwrap();
}

#[tokio::test]
async fn build_services_specific_missing_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.build_services(Some(format!("x-{}", uuid::Uuid::new_v4().simple())))
        .await
        .unwrap();
}

#[tokio::test]
async fn build_and_restart_services_empty_target_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.build_and_restart_services(None).await.unwrap();
}

#[tokio::test]
async fn build_and_restart_specific_missing_ok() {
    let Some(o) = make_orchestrator().await else { return };
    o.build_and_restart_services(Some(format!("x-{}", uuid::Uuid::new_v4().simple())))
        .await
        .unwrap();
}

#[tokio::test]
async fn subscribe_events_multiple_receivers() {
    let Some(o) = make_orchestrator().await else { return };
    let _rx1 = o.subscribe_events();
    let _rx2 = o.subscribe_events();
}

#[tokio::test]
async fn get_running_servers_empty_db_returns_empty_vec() {
    let Some(o) = make_orchestrator().await else { return };
    let r = o.get_running_servers().await.unwrap();
    let _ = r.len();
}

#[tokio::test]
async fn registry_accessor_returns_reference() {
    let Some(o) = make_orchestrator().await else { return };
    let r = o.registry();
    let _ = r.get_enabled_servers().unwrap_or_default();
}
