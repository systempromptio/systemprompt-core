//! DB-backed smoke tests for [`LifecycleOrchestrator`] accessors and
//! shutdown / health-check on missing services (no real spawn).

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::lifecycle::LifecycleOrchestrator;
use systemprompt_mcp::services::monitoring::MonitoringService;
use systemprompt_mcp::services::network::NetworkService;
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

async fn make_orchestrator() -> Option<(LifecycleOrchestrator, McpServerConfig)> {
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
    let database = DatabaseService::new(db, Arc::clone(&app_paths), registry);
    let lifecycle = LifecycleOrchestrator::new(
        ProcessService::new(),
        NetworkService::new(),
        database,
        MonitoringService::new(),
        app_paths,
    );

    let config = McpServerConfig {
        name: format!("ghost-{}", uuid::Uuid::new_v4().simple()),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: "nonexistent-bin".to_string(),
        enabled: true,
        display_in_web: true,
        port: 65530,
        crate_path: PathBuf::from("."),
        display_name: "ghost".to_string(),
        description: "no real server".to_string(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.0.1".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    };

    Some((lifecycle, config))
}

#[tokio::test]
async fn accessor_methods_return_inner_services() {
    let Some((life, _)) = make_orchestrator().await else {
        return;
    };
    let _ = life.process();
    let _ = life.network();
    let _ = life.database();
    let _ = life.monitoring();
    let _ = life.app_paths();
}

#[tokio::test]
async fn stop_server_on_missing_service_is_noop() {
    let Some((life, config)) = make_orchestrator().await else {
        return;
    };
    life.stop_server(&config).await.unwrap();
}

#[tokio::test]
async fn health_check_on_missing_service_returns_false() {
    let Some((life, config)) = make_orchestrator().await else {
        return;
    };
    let r = life.health_check(&config).await.unwrap();
    assert!(!r);
}

#[tokio::test]
async fn start_server_with_nonexistent_binary_returns_err() {
    let Some((life, config)) = make_orchestrator().await else {
        return;
    };
    let r = life.start_server(&config).await;
    assert!(r.is_err());
}
