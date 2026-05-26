//! Constructor tests for orchestrator handlers and the event bus.

use std::sync::Arc;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::lifecycle::LifecycleOrchestrator;
use systemprompt_mcp::services::monitoring::MonitoringService;
use systemprompt_mcp::services::network::NetworkService;
use systemprompt_mcp::services::orchestrator::{
    DatabaseSyncHandler, EventBus, HealthCheckHandler, LifecycleHandler, MonitoringHandler,
};
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn make_dependencies() -> Option<(LifecycleOrchestrator, DatabaseService, RegistryService)> {
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
    let database = DatabaseService::new(db, Arc::clone(&app_paths), registry.clone());
    let lifecycle = LifecycleOrchestrator::new(
        ProcessService::new(),
        NetworkService::new(),
        database.clone(),
        MonitoringService::new(),
        app_paths,
    );
    Some((lifecycle, database, registry))
}

#[tokio::test]
async fn lifecycle_handler_construction() {
    let Some((lifecycle, _db, registry)) = make_dependencies().await else {
        return;
    };
    let h = LifecycleHandler::new(lifecycle, registry);
    let _ = format!("{h:?}");
}

#[test]
fn monitoring_handler_construction() {
    let h = MonitoringHandler;
    let _ = format!("{h:?}");
}

#[test]
fn health_check_handler_new_and_with_restart_sender() {
    let bus = EventBus::new(10);
    let h = HealthCheckHandler::new().with_restart_sender(bus.sender());
    let _ = format!("{h:?}");
}

#[tokio::test]
async fn database_sync_handler_construction() {
    let Some((_lifecycle, database, _registry)) = make_dependencies().await else {
        return;
    };
    let h = DatabaseSyncHandler::new(database);
    let _ = format!("{h:?}");
}

#[test]
fn event_bus_construct_and_sender() {
    let bus = EventBus::new(10);
    let _ = bus.sender();
    let mut rx = bus.subscribe();
    drop(rx.try_recv());
}
