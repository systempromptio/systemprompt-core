//! Drives `McpToolLoader` end-to-end: permission filtering, DB-lag retry,
//! live tool listing against a scripted MCP endpoint, and gateway metadata
//! assembly via `create_mcp_extensions`.

use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_mcp::orchestration::McpToolLoader;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::AppPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, config_with_servers, default_tools_json, external_server_block,
    mount_mcp_endpoint, request_context, write_services_config,
};

struct Live {
    loader: McpToolLoader,
    database: DatabaseService,
    registry: RegistryService,
    server_name: String,
}

async fn live_setup(oauth_required: bool) -> Option<(Live, MockServer)> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;

    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let server_name = format!("ldr_{}", uuid::Uuid::new_v4().simple());
    let yaml = config_with_servers(&[external_server_block(&ExternalServerSpec {
        name: &server_name,
        endpoint: &format!("{}/mcp", mock.uri()),
        oauth_required,
        enabled: true,
    })]);
    write_services_config(bootstrap, &yaml);

    let registry = RegistryService::new(fixture_user_id());
    let app_paths = Arc::new(AppPaths::from_profile(&profile_paths(bootstrap)).ok()?);
    let database = DatabaseService::new(db.clone(), app_paths, registry.clone());
    let loader = McpToolLoader::new(&db, registry.clone()).ok()?;

    Some((
        Live {
            loader,
            database,
            registry,
            server_name,
        },
        mock,
    ))
}

fn profile_paths(
    bootstrap: &systemprompt_test_fixtures::TestBootstrap,
) -> systemprompt_models::profile::PathsConfig {
    systemprompt_models::profile::PathsConfig {
        system: bootstrap.system_path.display().to_string(),
        services: bootstrap.services_path.display().to_string(),
        bin: bootstrap.bin_path.display().to_string(),
        web_path: None,
        storage: Some(bootstrap.storage_path.display().to_string()),
        geoip_database: None,
    }
}

#[tokio::test]
async fn load_tools_for_running_server_returns_tools() {
    let Some((live, _mock)) = live_setup(false).await else {
        return;
    };
    let config = live
        .registry
        .get_server(&live.server_name)
        .expect("server in registry");
    live.database
        .register_service(&config, std::process::id())
        .await
        .expect("service registered");

    let tools_by_server = live
        .loader
        .load_tools_for_servers(
            std::slice::from_ref(&live.server_name),
            &request_context("ldr"),
        )
        .await
        .expect("tools load");

    let tools = tools_by_server
        .get(&live.server_name)
        .expect("server present");
    assert_eq!(tools.len(), 2);

    live.database
        .unregister_service(&live.server_name)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn unregistered_server_exhausts_db_retry() {
    let Some((live, _mock)) = live_setup(false).await else {
        return;
    };

    let err = live
        .loader
        .load_server_tools(&live.server_name, &request_context("ldr-miss"))
        .await
        .expect_err("missing service row");
    assert!(err.to_string().contains("not found in services database"));
}

#[tokio::test]
async fn scoped_server_is_skipped_for_anonymous_caller() {
    let Some((live, _mock)) = live_setup(true).await else {
        return;
    };
    let bootstrap = ensure_test_bootstrap();
    let yaml = config_with_servers(&[external_server_block(&ExternalServerSpec {
        name: &live.server_name,
        endpoint: "http://127.0.0.1:59997/mcp",
        oauth_required: true,
        enabled: true,
    })])
    .replace("scopes: []", "scopes: [admin]");
    write_services_config(bootstrap, &yaml);

    let tools_by_server = live
        .loader
        .load_tools_for_servers(
            std::slice::from_ref(&live.server_name),
            &request_context("ldr-skip"),
        )
        .await
        .expect("load succeeds with skips");
    assert!(tools_by_server.is_empty());
}

#[tokio::test]
async fn invalid_jwt_fails_permission_extraction() {
    let Some((live, _mock)) = live_setup(false).await else {
        return;
    };

    let context = request_context("ldr-jwt").with_auth_token("not-a-jwt".to_owned());
    let err = live
        .loader
        .load_tools_for_servers(std::slice::from_ref(&live.server_name), &context)
        .await
        .expect_err("garbage JWT rejected");
    assert!(err.to_string().contains("JWT validation failed"));
}

#[tokio::test]
async fn create_mcp_extensions_reports_status_and_unknown_servers() {
    let Some((live, _mock)) = live_setup(false).await else {
        return;
    };
    let config = live
        .registry
        .get_server(&live.server_name)
        .expect("server in registry");
    live.database
        .register_service(&config, std::process::id())
        .await
        .expect("service registered");

    let unknown = format!("ghost_{}", uuid::Uuid::new_v4().simple());
    let servers = vec![live.server_name.clone(), unknown.clone()];
    let infos = live
        .loader
        .create_mcp_extensions(&servers, "http://gw.example", &request_context("ldr-ext"))
        .await
        .expect("extensions assemble");

    assert_eq!(infos.len(), 2);
    let known = infos
        .iter()
        .find(|i| i.name == live.server_name)
        .expect("known server");
    assert_eq!(known.auth, "anon");
    assert_eq!(known.status, "running");
    assert!(known.endpoint.contains("/api/v1/mcp/"));
    assert_eq!(known.tools.as_ref().map(Vec::len), Some(2));

    let ghost = infos.iter().find(|i| i.name == unknown).expect("ghost");
    assert_eq!(ghost.auth, "unknown");
    assert_eq!(ghost.status, "not_in_config");
    assert!(ghost.tools.is_none());

    live.database
        .unregister_service(&live.server_name)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn create_mcp_extensions_empty_input_short_circuits() {
    let _ = ensure_test_bootstrap();
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let loader = McpToolLoader::new(&db, RegistryService::new(UserId::new("owner-empty")))
        .expect("loader builds");

    let infos = loader
        .create_mcp_extensions(&[], "http://gw.example", &request_context("ldr-empty"))
        .await
        .expect("empty ok");
    assert!(infos.is_empty());
    let _ = loader.service_manager();
}

#[tokio::test]
async fn stopped_service_row_is_reported_not_running() {
    let Some((live, _mock)) = live_setup(false).await else {
        return;
    };
    let config = live
        .registry
        .get_server(&live.server_name)
        .expect("server in registry");
    live.database
        .register_service(&config, std::process::id())
        .await
        .expect("service registered");
    live.database
        .update_service_status(&live.server_name, "stopped")
        .await
        .expect("status updated");

    let err = live
        .loader
        .load_server_tools(&live.server_name, &request_context("ldr-stop"))
        .await
        .expect_err("stopped service rejected");

    live.database
        .unregister_service(&live.server_name)
        .await
        .expect("cleanup");

    assert!(err.to_string().contains("is not running"));
    assert!(err.to_string().contains("stopped"));
}

#[tokio::test]
async fn scoped_server_metadata_advertises_first_scope_without_tools() {
    let Some((live, _mock)) = live_setup(true).await else {
        return;
    };
    let bootstrap = ensure_test_bootstrap();
    let yaml = config_with_servers(&[external_server_block(&ExternalServerSpec {
        name: &live.server_name,
        endpoint: "http://127.0.0.1:59996/mcp",
        oauth_required: true,
        enabled: true,
    })])
    .replace("scopes: []", "scopes: [admin, user]");
    write_services_config(bootstrap, &yaml);

    let infos = live
        .loader
        .create_mcp_extensions(
            std::slice::from_ref(&live.server_name),
            "http://gw.example",
            &request_context("ldr-scope"),
        )
        .await
        .expect("extensions assemble");

    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].auth, "admin");
    assert_eq!(infos[0].status, "not_started");
    assert!(infos[0].tools.is_none());
}
