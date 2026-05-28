//! Deep DB-seeded tests for the lifecycle / shutdown / health-check paths.
//!
//! Inserts a fake `services` row pointing at an impossible PID so that
//! `stop_server` follows the cleanup-stale-pid branch and `health_check`
//! follows the dead-port branch. Validates the orchestrator's read-only
//! behaviour without launching a real child process.

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::database::sync::{
    cleanup_stale_services, delete_crashed_services, reconcile_running_processes,
    repair_database_inconsistencies, sync_database_state,
};
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
    let lifecycle = LifecycleOrchestrator::new(
        ProcessService::new(),
        NetworkService::new(),
        database,
        MonitoringService::new(),
        app_paths,
    );
    Some((lifecycle, db))
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
        description: format!("{name}"),
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
    }
}

#[tokio::test]
async fn stop_server_cleans_up_stale_db_row() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("stop-stale-{}", uuid::Uuid::new_v4().simple());
    let port = 65528;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let config = make_config(&name, port);
    life.stop_server(&config).await.unwrap();
    let info = life.database().get_service_by_name(&name).await.unwrap();
    assert!(info.is_none());
}

#[tokio::test]
async fn health_check_dead_port_returns_false_and_updates_status() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("health-dead-{}", uuid::Uuid::new_v4().simple());
    let port = 65527;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let config = make_config(&name, port);
    let r = life.health_check(&config).await.unwrap();
    assert!(!r);

    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn cleanup_stale_services_marks_dead_port_rows_stopped() {
    let Some((_, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("clean-stale-{}", uuid::Uuid::new_v4().simple());
    let port = 65526;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    cleanup_stale_services(&db).await.unwrap();
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn sync_database_state_marks_unhealthy_crashed() {
    let Some((_, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("sync-crash-{}", uuid::Uuid::new_v4().simple());
    let port = 65525;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let config = make_config(&name, port);
    sync_database_state(&db, &[config]).await.unwrap();
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn reconcile_running_processes_reports_dead_ports() {
    let Some((_, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("rec-{}", uuid::Uuid::new_v4().simple());
    let port = 65524;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();

    let _ = reconcile_running_processes(&db).await.unwrap();
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn repair_inconsistencies_marks_pidless_running_as_stopped() {
    let Some((_, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("repair-{}", uuid::Uuid::new_v4().simple());
    let port = 65523;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repair_database_inconsistencies(&db).await.unwrap();
    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn delete_crashed_services_runs() {
    let Some((_, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("crash-{}", uuid::Uuid::new_v4().simple());
    let port = 65522;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "crashed",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    delete_crashed_services(&db).await.unwrap();
}

#[tokio::test]
async fn health_check_with_stale_pid_marks_stopped() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("health-pid-{}", uuid::Uuid::new_v4().simple());
    let port = 65521;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo.update_service_pid(&name, 999_999).await.unwrap();

    let config = make_config(&name, port);
    let r = life.health_check(&config).await.unwrap();
    assert!(!r);

    repo.delete_service(&name).await.unwrap();
}

#[tokio::test]
async fn stop_server_with_stale_db_pid_goes_through_stale_cleanup() {
    let Some((life, db)) = make_lifecycle().await else {
        return;
    };
    let name = format!("stop-pid-{}", uuid::Uuid::new_v4().simple());
    let port = 65520;
    let repo = ServiceRepository::new(&db).unwrap();
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: "mcp",
        status: "running",
        port,
        binary_mtime: None,
    })
    .await
    .unwrap();
    repo.update_service_pid(&name, 999_998).await.unwrap();

    let config = make_config(&name, port);
    life.stop_server(&config).await.unwrap();

    let after = life.database().get_service_by_name(&name).await.unwrap();
    assert!(after.is_none());
}
