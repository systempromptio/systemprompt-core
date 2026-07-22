//! Lifecycle paths that need a live endpoint or a live child process: the
//! healthy/unhealthy health-check verdicts, graceful shutdown of a running
//! PID, and the restart clean-state sweep.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
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
use wiremock::MockServer;

use crate::harness::{default_tools_json, mount_mcp_endpoint};

async fn make_lifecycle() -> Option<(LifecycleOrchestrator, systemprompt_database::DbPool)> {
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
    let database = DatabaseService::new(db.clone(), Arc::clone(&app_paths), registry);
    Some((
        LifecycleOrchestrator::new(
            ProcessService::new(),
            NetworkService::new(),
            database,
            MonitoringService::new(),
            app_paths,
        ),
        db,
    ))
}

fn make_config(name: &str, port: u16) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: name.to_owned(),
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
        version: "0.0.1".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

async fn seed_service(
    db: &systemprompt_database::DbPool,
    name: &str,
    port: u16,
) -> ServiceRepository {
    let repo = ServiceRepository::new(db).unwrap();
    repo.create_service(CreateServiceInput {
        name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo
}

#[tokio::test]
async fn health_check_live_mcp_endpoint_reports_healthy() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let port = mock.address().port();

    let name = format!("hc-live-{}", uuid::Uuid::new_v4().simple());
    let repo = seed_service(&db, &name, port).await;

    let healthy = life.health_check(&make_config(&name, port)).await.unwrap();

    let status = repo
        .find_service_by_name(&name)
        .await
        .unwrap()
        .unwrap()
        .status;
    repo.delete_service(&name).await.unwrap();

    assert!(healthy);
    assert_eq!(status, "running");
}

#[tokio::test]
async fn health_check_non_mcp_listener_marks_service_error() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let mock = MockServer::start().await;
    let port = mock.address().port();

    let name = format!("hc-err-{}", uuid::Uuid::new_v4().simple());
    let repo = seed_service(&db, &name, port).await;

    let healthy = life.health_check(&make_config(&name, port)).await.unwrap();

    let status = repo
        .find_service_by_name(&name)
        .await
        .unwrap()
        .unwrap()
        .status;
    repo.delete_service(&name).await.unwrap();

    assert!(!healthy);
    assert_eq!(status, "error");
}

#[tokio::test]
async fn stop_server_terminates_registered_live_child_and_finalizes_row() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };

    let name = format!("stop-live-{}", uuid::Uuid::new_v4().simple());
    let port = 65401;
    let mut child = Command::new("sleep")
        .arg("30")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("MCP_SERVICE_ID", &name)
        .spawn()
        .expect("spawn sleep");

    let repo = seed_service(&db, &name, port).await;
    repo.update_service_pid(&name, i32::try_from(child.id()).unwrap())
        .await
        .unwrap();

    life.stop_server(&make_config(&name, port)).await.unwrap();

    let row = repo.find_service_by_name(&name).await.unwrap().unwrap();
    repo.delete_service(&name).await.unwrap();

    assert_eq!(row.status, "stopped");
    assert!(row.pid.is_none());
    assert!(!child.wait().expect("child reaped").success());
}

#[tokio::test]
async fn restart_server_sweeps_stale_running_row_then_fails_on_missing_binary() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };

    let name = format!("restart-{}", uuid::Uuid::new_v4().simple());
    let port = 65402;
    let repo = seed_service(&db, &name, port).await;

    let result = life.restart_server(&make_config(&name, port)).await;

    let row = repo.find_service_by_name(&name).await.unwrap();
    if let Some(row) = &row {
        assert_ne!(row.status, "running");
    }
    repo.delete_service(&name).await.ok();

    assert!(result.is_err(), "startup cannot succeed without a binary");
}
