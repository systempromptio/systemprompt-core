//! Drives `McpToolLoader` end-to-end: permission filtering, DB-lag retry,
//! live tool listing against a scripted MCP endpoint, and gateway metadata
//! assembly via `create_mcp_extensions`.


use systemprompt_identifiers::UserId;
use systemprompt_mcp::orchestration::McpToolLoader;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, bootstrap_with_services, config_with_servers, default_tools_json,
    external_server_block, mount_mcp_endpoint, request_context,
};

struct Live {
    loader: McpToolLoader,
    server_name: String,
}

async fn reserve_mcp_listener() -> tokio::net::TcpListener {
    for port in 5000..6000 {
        if let Ok(listener) =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await
        {
            return listener;
        }
    }
    panic!("no isolated MCP test port available in 5000..5999");
}


fn register_internal_server_extension(
    bootstrap: &systemprompt_test_fixtures::TestBootstrap,
    server_name: &str,
) {
    let binary = format!("{server_name}-bin");
    let extension = bootstrap.system_path.join("extensions").join(server_name);
    std::fs::create_dir_all(&extension).expect("internal extension directory");
    std::fs::write(
        extension.join("manifest.yaml"),
        format!(
            "extension:\n  type: mcp\n  name: {server_name}\n  binary: {binary}\n  description: loader boundary fixture\n  enabled: true\n"
        ),
    )
    .expect("internal extension manifest");
}

async fn live_setup_or_skip(oauth_required: bool) -> Option<(Live, MockServer)> {
    live_setup_scoped_or_skip(oauth_required, "").await
}

async fn live_setup_scoped_or_skip(
    oauth_required: bool,
    scopes: &str,
) -> Option<(Live, MockServer)> {
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
    })])
    .replace("scopes: []", &format!("scopes: [{scopes}]"));
    bootstrap_with_services(&yaml);

    let registry = RegistryService::new(fixture_user_id());
    let loader = McpToolLoader::new(
        systemprompt_database::ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        )
        .expect("service repository"),
        registry,
    );

    Some((
        Live {
            loader,
            server_name,
        },
        mock,
    ))
}

#[tokio::test]
async fn external_server_loads_tools_without_a_service_row() {
    let Some((live, _mock)) = live_setup_or_skip(false).await else {
        return;
    };

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
}

#[tokio::test]
async fn stopped_internal_server_has_no_row_to_load_from() {
    let Some((live, _mock)) = live_setup_or_skip(false).await else {
        return;
    };
    let internal = format!("ldr_int_{}", uuid::Uuid::new_v4().simple());

    let err = live
        .loader
        .load_server_tools(&internal, &request_context("ldr-miss"))
        .await
        .expect_err("unknown server");
    assert!(err.to_string().contains(&internal), "{err}");
}

#[tokio::test]
async fn scoped_server_is_skipped_for_anonymous_caller() {
    let Some((live, _mock)) = live_setup_scoped_or_skip(true, "admin").await else {
        return;
    };

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
    let Some((live, _mock)) = live_setup_or_skip(false).await else {
        return;
    };

    let context = request_context("ldr-jwt").with_auth_token("not-a-jwt".to_owned());
    let err = live
        .loader
        .load_tools_for_servers(std::slice::from_ref(&live.server_name), &context)
        .await
        .expect_err("garbage JWT rejected");
    assert!(err.to_string().contains("token rejected"));
}

#[tokio::test]
async fn create_mcp_extensions_reports_status_and_unknown_servers() {
    let Some((live, _mock)) = live_setup_or_skip(false).await else {
        return;
    };

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
    assert_eq!(known.status, "external");
    assert!(known.endpoint.contains("/api/v1/mcp/"));
    assert_eq!(known.tools.as_ref().map(Vec::len), Some(2));

    let ghost = infos.iter().find(|i| i.name == unknown).expect("ghost");
    assert_eq!(ghost.auth, "unknown");
    assert_eq!(ghost.status, "not_in_config");
    assert!(ghost.tools.is_none());
}

#[tokio::test]
async fn create_mcp_extensions_empty_input_short_circuits() {
    let _bootstrap = bootstrap_with_services("{}\n");
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let loader = McpToolLoader::new(
        systemprompt_database::ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("test-instance"),
        )
        .expect("service repository"),
        RegistryService::new(UserId::new("owner-empty")),
    );

    let infos = loader
        .create_mcp_extensions(&[], "http://gw.example", &request_context("ldr-empty"))
        .await
        .expect("empty ok");
    assert!(infos.is_empty());
    let _ = loader.service_manager();
}

#[tokio::test]
async fn scoped_server_metadata_advertises_first_scope_without_tools() {
    let Some((live, _mock)) = live_setup_scoped_or_skip(true, "admin, user").await else {
        return;
    };

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
    assert_eq!(infos[0].status, "external");
    assert!(infos[0].tools.is_none());
}

#[tokio::test]
async fn configured_internal_server_with_stopped_row_is_rejected_without_transport_dispatch() {
    use crate::harness::{config_with_servers, internal_server_block};
    use systemprompt_database::{CreateServiceInput, ServiceRepository};

    let server_name = format!("stopped_{}", uuid::Uuid::new_v4().simple());
    let listener = reserve_mcp_listener().await;
    let port = listener.local_addr().expect("listener address").port();
    let yaml = config_with_servers(&[internal_server_block(&server_name, port)]);
    let bootstrap = bootstrap_with_services(&yaml);
    register_internal_server_extension(bootstrap, &server_name);
    let url = fixture_database_url().expect("fixture database URL");
    let db = fixture_db_pool(&url).await.expect("fixture pool");
    let repo = ServiceRepository::new(
        &db,
        systemprompt_identifiers::InstanceId::new("loader-stopped-instance"),
    )
    .expect("service repository");
    repo.create_service(CreateServiceInput {
        name: &server_name,
        module_name: "mcp",
        status: "stopped",
        port,
        binary_mtime: None,
    })
    .await
    .expect("stopped service row");
    let loader = McpToolLoader::new(repo.clone(), RegistryService::new(fixture_user_id()));

    let error = loader
        .load_server_tools(&server_name, &request_context("ldr-stopped"))
        .await
        .expect_err("stopped internal server must not dispatch a tool-list request");
    let message = error.to_string();
    assert!(message.contains(&server_name), "{message}");
    assert!(
        message.contains("not running") && message.contains("stopped"),
        "{message}"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), listener.accept())
            .await
            .is_err(),
        "a stopped service must be rejected before transport dispatch"
    );
    repo.delete_service(&server_name)
        .await
        .expect("fixture cleanup");
}

#[tokio::test]
async fn internal_server_database_failure_is_distinguished_from_replication_lag() {
    use crate::harness::{config_with_servers, internal_server_block};

    let server_name = format!("db_error_{}", uuid::Uuid::new_v4().simple());
    let listener = reserve_mcp_listener().await;
    let port = listener.local_addr().expect("listener address").port();
    let bootstrap = bootstrap_with_services(&config_with_servers(&[internal_server_block(
        &server_name,
        port,
    )]));
    register_internal_server_extension(bootstrap, &server_name);
    let db = systemprompt_test_fixtures::closed_db_pool().await;
    let loader = McpToolLoader::new(
        systemprompt_database::ServiceRepository::new(
            &db,
            systemprompt_identifiers::InstanceId::new("loader-closed-instance"),
        )
        .expect("service repository"),
        RegistryService::new(fixture_user_id()),
    );

    let error = loader
        .load_server_tools(&server_name, &request_context("ldr-db-error"))
        .await
        .expect_err("closed database must fail without lag retries");
    let message = error.to_string();
    assert!(message.contains(&server_name), "{message}");
    assert!(
        message.contains("Database error querying MCP server"),
        "{message}"
    );
    assert!(message.contains("not replication lag"), "{message}");
    assert!(!message.contains("after 3 retries"), "{message}");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), listener.accept())
            .await
            .is_err(),
        "database failure must be returned before transport dispatch"
    );
}
