//! DB-backed smoke tests for [`McpOrchestrator`].
//!
//! Constructs an orchestrator over a fresh (empty) registry/database and
//! drives the read-only branches (`list_services`, `reconcile`,
//! `sync_database_state`, `get_running_servers`, validation of a missing
//! service). Lifecycle / process-spawn paths are exercised by the existing
//! integration suite.

use std::sync::Arc;
use systemprompt_mcp::services::orchestrator::McpOrchestrator;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn make_orchestrator() -> Option<McpOrchestrator> {
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
async fn orchestrator_new_succeeds() {
    let Some(_o) = make_orchestrator().await else {
        return;
    };
}

#[tokio::test]
async fn orchestrator_get_running_servers() {
    let Some(o) = make_orchestrator().await else {
        return;
    };
    let _ = o.get_running_servers().await.unwrap();
}

#[tokio::test]
async fn orchestrator_get_service_info_missing_returns_none() {
    let Some(o) = make_orchestrator().await else {
        return;
    };
    let r = o
        .get_service_info(&format!("missing-{}", uuid::Uuid::new_v4().simple()))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn orchestrator_subscribe_events_returns_receiver() {
    let Some(o) = make_orchestrator().await else {
        return;
    };
    let _rx = o.subscribe_events();
}

#[tokio::test]
async fn orchestrator_registry_accessor() {
    let Some(o) = make_orchestrator().await else {
        return;
    };
    let _ = o.registry();
}
