//! Live client-transport coverage for `systemprompt_mcp`.
//!
//! Drives the real streamable-HTTP client (`validate_connection*`,
//! `perform_health_check`, `check_service_health`) against an in-process
//! `rmcp` server stood up by [`crate::mock_server`]. Exercises the connect →
//! `initialize` → `tools/list` → `tools/call` → cancel handshake over genuine
//! HTTP/SSE rather than a wire mock.

use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::client::{validate_connection, validate_connection_by_url};
use systemprompt_mcp::services::monitoring::health::{
    HealthStatus, check_service_health, perform_health_check,
};
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::{McpServerType, OAuthRequirement};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_user_id};

use crate::mock_server::start_echo_mcp_server;

fn external_config(name: &str, remote_endpoint: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::External,
        binary: "echo".to_owned(),
        enabled: true,
        display_in_web: true,
        port: 0,
        crate_path: PathBuf::from("/tmp"),
        display_name: name.to_owned(),
        description: "live test server".to_owned(),
        capabilities: vec!["tools".to_owned()],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: Vec::<Permission>::new(),
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "1.0.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: remote_endpoint.to_owned(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn internal_config(name: &str, host: &str, port: u16) -> McpServerConfig {
    let mut config = external_config(name, "");
    config.server_type = McpServerType::Internal;
    config.host = host.to_owned();
    config.port = port;
    config
}

#[tokio::test]
async fn validate_connection_by_url_against_live_server_lists_tools() {
    ensure_test_bootstrap();
    let server = start_echo_mcp_server("echo").await;

    let result = validate_connection_by_url("live-echo", &server.url)
        .await
        .expect("validation must not error");

    assert!(result.success, "validation should succeed: {result:?}");
    assert_eq!(result.tools_count, 1, "echo server advertises one tool");
    assert_eq!(result.validation_type, "mcp_validated");
    let info = result.server_info.expect("peer info present");
    assert_eq!(info.server_name, "echo-mcp-test-server");
    assert_eq!(info.version, "9.9.9");
}

#[tokio::test]
async fn validate_connection_by_url_unreachable_reports_failure() {
    ensure_test_bootstrap();

    let result = validate_connection_by_url("dead", "http://127.0.0.1:1/mcp")
        .await
        .expect("validation wraps connection errors, does not panic");

    assert!(!result.success);
    assert!(
        matches!(
            result.validation_type.as_str(),
            "connection_failed" | "timeout"
        ),
        "unexpected validation_type: {}",
        result.validation_type
    );
}

#[tokio::test]
async fn validate_connection_host_port_against_live_server() {
    ensure_test_bootstrap();
    let server = start_echo_mcp_server("echo").await;

    let result = validate_connection("live-echo", &server.host, server.port).await;

    match result {
        Ok(r) => {
            assert!(!r.service_name.is_empty());
        },
        Err(e) => panic!("validate_connection errored: {e}"),
    }
}

#[tokio::test]
async fn perform_health_check_external_server_is_healthy() {
    ensure_test_bootstrap();
    let server = start_echo_mcp_server("echo").await;
    let config = external_config("live-external", &server.url);

    let health = perform_health_check(&config)
        .await
        .expect("health check must not error");

    assert!(
        matches!(
            health.status,
            HealthStatus::Healthy | HealthStatus::Degraded
        ),
        "reachable MCP server with a tool should connect: {health:?}"
    );
    assert_eq!(health.details.tools_available, 1);
    assert!(!health.details.requires_auth);
}

#[tokio::test]
async fn check_service_health_internal_server_path() {
    ensure_test_bootstrap();
    let server = start_echo_mcp_server("echo").await;
    let config = internal_config("live-internal", &server.host, server.port);

    let status = check_service_health(&config)
        .await
        .expect("health check must not error");

    assert!(
        matches!(status, HealthStatus::Healthy | HealthStatus::Degraded),
        "internal probe of a live server should connect: {status:?}"
    );
}

#[tokio::test]
async fn perform_health_check_unreachable_external_is_unhealthy() {
    ensure_test_bootstrap();
    let config = external_config("dead-external", "http://127.0.0.1:1/mcp");

    let health = perform_health_check(&config)
        .await
        .expect("health check wraps errors");

    assert_eq!(health.status, HealthStatus::Unhealthy);
    assert_eq!(health.details.tools_available, 0);
}
