//! Constructor tests for orchestrator subscribers and the event bus.

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::lifecycle::LifecycleService;
use systemprompt_mcp::services::monitoring::MonitoringService;
use systemprompt_mcp::services::network::NetworkService;
use systemprompt_mcp::services::orchestrator::{
    DatabaseSyncSubscriber, EventBus, LifecycleSubscriber, MonitoringSubscriber,
};
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_test_fixtures::{fixture_user_id, test_db_pool};

async fn make_dependencies() -> (LifecycleService, DatabaseService, RegistryService) {
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
    let database = DatabaseService::new(
        systemprompt_database::ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        ),
        Arc::clone(&app_paths),
        registry.clone(),
    );
    let lifecycle = LifecycleService::new(
        ProcessService::new(),
        NetworkService::new(),
        database.clone(),
        MonitoringService::new(),
        app_paths,
    );
    (lifecycle, database, registry)
}

#[tokio::test]
async fn lifecycle_handler_construction() {
    let h = LifecycleSubscriber;
    let _ = format!("{h:?}");
}

#[test]
fn monitoring_handler_construction() {
    let h = MonitoringSubscriber;
    let _ = format!("{h:?}");
}

#[tokio::test]
async fn database_sync_handler_construction() {
    let (_lifecycle, database, _registry) = make_dependencies().await;
    let h = DatabaseSyncSubscriber::new(database);
    let _ = format!("{h:?}");
}

#[test]
fn event_bus_construct_and_subscribe() {
    let bus = EventBus::new(10);
    let mut rx = bus.subscribe();
    drop(rx.try_recv());
}
