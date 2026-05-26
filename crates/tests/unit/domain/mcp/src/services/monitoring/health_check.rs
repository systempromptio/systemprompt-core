//! Unit tests covering the `perform_health_check` / `HealthCheckResult`
//! constructors with synthetic [`McpServerConfig`] values and unreachable
//! endpoints.

use std::path::PathBuf;
use systemprompt_mcp::services::client::{McpConnectionResult, McpProtocolInfo};
use systemprompt_mcp::services::monitoring::health::{
    HealthCheckResult, HealthStatus, check_service_health, perform_health_check,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn unused_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn make_config(name: &str, server_type: McpServerType, port: u16, endpoint: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: format!("{name} MCP Server"),
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
        version: "0.1.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: endpoint.to_owned(),
    }
}

#[test]
fn from_connection_result_success_under_1s_is_healthy() {
    let config = make_config("h1", McpServerType::Internal, 9000, "");
    let result = McpConnectionResult {
        service_name: "h1".to_owned(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: Some(McpProtocolInfo {
            server_name: "h1".to_owned(),
            version: "1.0".to_owned(),
            protocol_version: "2024-11-05".to_owned(),
        }),
        tools_count: 3,
        validation_type: "mcp_validated".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Healthy));
    assert_eq!(hc.details.tools_available, 3);
    assert_eq!(hc.details.server_version.as_deref(), Some("1.0"));
}

#[test]
fn from_connection_result_slow_is_degraded() {
    let config = make_config("h2", McpServerType::Internal, 9001, "");
    let result = McpConnectionResult {
        service_name: "h2".to_owned(),
        success: true,
        error_message: None,
        connection_time_ms: 2500,
        server_info: None,
        tools_count: 0,
        validation_type: "mcp_validated".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Degraded));
}

#[test]
fn from_connection_result_auth_required_is_healthy() {
    let config = make_config("h3", McpServerType::Internal, 9002, "");
    let result = McpConnectionResult {
        service_name: "h3".to_owned(),
        success: false,
        error_message: Some("auth".to_owned()),
        connection_time_ms: 50,
        server_info: None,
        tools_count: 0,
        validation_type: "auth_required".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Healthy));
}

#[test]
fn from_connection_result_connection_failed_is_unhealthy() {
    let config = make_config("h4", McpServerType::Internal, 9003, "");
    let result = McpConnectionResult {
        service_name: "h4".to_owned(),
        success: false,
        error_message: Some("nope".to_owned()),
        connection_time_ms: 5000,
        server_info: None,
        tools_count: 0,
        validation_type: "connection_failed".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Unhealthy));
}

#[test]
fn from_connection_result_port_unavailable_is_unhealthy() {
    let config = make_config("h5", McpServerType::Internal, 9004, "");
    let result = McpConnectionResult {
        service_name: "h5".to_owned(),
        success: false,
        error_message: Some("port".to_owned()),
        connection_time_ms: 0,
        server_info: None,
        tools_count: 0,
        validation_type: "port_unavailable".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Unhealthy));
}

#[test]
fn from_connection_result_unknown_type_is_unknown() {
    let config = make_config("h6", McpServerType::Internal, 9005, "");
    let result = McpConnectionResult {
        service_name: "h6".to_owned(),
        success: false,
        error_message: None,
        connection_time_ms: 0,
        server_info: None,
        tools_count: 0,
        validation_type: "anything_else".to_owned(),
    };
    let hc = HealthCheckResult::from_connection_result(result, &config);
    assert!(matches!(hc.status, HealthStatus::Unknown));
}

#[test]
fn unhealthy_constructor() {
    let config = make_config("u", McpServerType::Internal, 9006, "");
    let hc = HealthCheckResult::unhealthy(&config, "boom".to_owned());
    assert!(matches!(hc.status, HealthStatus::Unhealthy));
    assert_eq!(hc.latency_ms, 0);
    assert_eq!(hc.details.error_message.as_deref(), Some("boom"));
    assert_eq!(hc.details.tools_available, 0);
}

#[tokio::test]
async fn perform_health_check_internal_unreachable_returns_unhealthy() {
    let port = unused_port();
    let config = make_config("internal-unreach", McpServerType::Internal, port, "");
    let result = perform_health_check(&config).await.unwrap();
    assert!(matches!(
        result.status,
        HealthStatus::Unhealthy | HealthStatus::Unknown
    ));
}

#[tokio::test]
async fn perform_health_check_external_invalid_url_returns_unhealthy() {
    let config = make_config("external-bad", McpServerType::External, 0, "not-a-url");
    let result = perform_health_check(&config).await.unwrap();
    assert!(!matches!(result.status, HealthStatus::Healthy));
}

#[tokio::test]
async fn check_service_health_returns_status_for_unreachable() {
    let port = unused_port();
    let config = make_config("svc-h", McpServerType::Internal, port, "");
    let status = check_service_health(&config).await.unwrap();
    assert!(!matches!(status, HealthStatus::Healthy));
}
