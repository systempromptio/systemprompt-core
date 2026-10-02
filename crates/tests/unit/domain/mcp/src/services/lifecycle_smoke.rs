//! DB-backed smoke tests for [`LifecycleService`] accessors and
//! shutdown / health-check on missing services (no real spawn).

use crate::harness::unique_instance;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::lifecycle::LifecycleService;
use systemprompt_mcp::services::monitoring::MonitoringService;
use systemprompt_mcp::services::network::NetworkService;
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{fixture_user_id, test_db_pool};

async fn make_orchestrator() -> (LifecycleService, McpServerConfig) {
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
    let database = DatabaseService::new(
        systemprompt_database::ServiceRepository::new(&db, unique_instance()),
        Arc::clone(&app_paths),
        registry,
    );
    let lifecycle = LifecycleService::new(
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
        binary: Some("nonexistent-bin".to_string()),
        enabled: true,
        display_in_web: true,
        port: Some(65530),
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
            ema: false,
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

    (lifecycle, config)
}

#[tokio::test]
async fn accessor_methods_return_inner_services() {
    let (life, _) = make_orchestrator().await;
    let _ = life.process();
    let _ = life.network();
    let _ = life.database();
    let _ = life.monitoring();
    let _ = life.app_paths();
}

#[tokio::test]
async fn stop_server_on_missing_service_is_noop() {
    let (life, config) = make_orchestrator().await;
    life.stop_server(&config).await.unwrap();
}

#[tokio::test]
async fn health_check_on_missing_service_returns_false() {
    let (life, config) = make_orchestrator().await;
    let r = life.health_check(&config).await.unwrap();
    assert!(!r);
}

#[tokio::test]
async fn start_server_with_nonexistent_binary_returns_err() {
    let (life, config) = make_orchestrator().await;
    let r = life.start_server(&config).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn start_server_rejects_external_without_spawning() {
    let (life, mut config) = make_orchestrator().await;
    config.server_type = McpServerType::External;
    config.binary = None;
    config.port = None;
    config.remote_endpoint = "https://api.salesforce.com/platform/mcp/v1".to_string();

    let err = life
        .start_server(&config)
        .await
        .expect_err("external server must not be spawned");

    assert!(
        err.to_string().contains("external"),
        "guard error should name the external invariant, got: {err}"
    );
}
