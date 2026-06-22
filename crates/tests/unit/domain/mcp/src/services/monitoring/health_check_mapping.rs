// Branch coverage for HealthCheckResult::from_connection_result and
// ::unhealthy — the connection-probe -> health-verdict mapping. Pure logic;
// no live server. McpConnectionResult and McpServerConfig have public fields.

use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::client::McpConnectionResult;
use systemprompt_mcp::services::monitoring::health::{HealthCheckResult, HealthStatus};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::OAuthRequirement;
use systemprompt_test_fixtures::fixture_user_id;

fn config(name: &str, oauth_required: bool) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        owner: fixture_user_id(),
        server_type: Default::default(),
        binary: "test-binary".to_string(),
        enabled: true,
        display_in_web: true,
        port: 8080,
        crate_path: PathBuf::from("/path"),
        display_name: "Test".to_string(),
        description: "desc".to_string(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: oauth_required,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "1.0.0".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn conn_result(
    success: bool,
    connection_time_ms: u32,
    validation_type: &str,
    error_message: Option<&str>,
    tools_count: usize,
) -> McpConnectionResult {
    McpConnectionResult {
        service_name: "svc".to_string(),
        success,
        error_message: error_message.map(String::from),
        connection_time_ms,
        server_info: None,
        tools_count,
        validation_type: validation_type.to_string(),
    }
}

#[test]
fn success_fast_is_healthy() {
    let result = conn_result(true, 50, "success", None, 7);
    let cfg = config("svc-a", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Healthy);
    assert_eq!(hc.latency_ms, 50);
    assert_eq!(hc.details.tools_available, 7);
    assert!(hc.connection_result.is_some());
}

#[test]
fn success_slow_is_degraded() {
    let result = conn_result(true, 1500, "success", None, 1);
    let cfg = config("svc-b", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Degraded);
    assert_eq!(hc.latency_ms, 1500);
}

#[test]
fn success_exactly_at_threshold_is_degraded() {
    // 1000ms is NOT < 1000, so it falls to the degraded branch.
    let result = conn_result(true, 1000, "success", None, 0);
    let cfg = config("svc-thr", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Degraded);
}

#[test]
fn failure_auth_required_is_healthy() {
    let result = conn_result(false, 10, "auth_required", Some("401"), 0);
    let cfg = config("svc-auth", true);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Healthy);
    assert!(hc.details.requires_auth);
}

#[test]
fn failure_port_unavailable_is_unhealthy() {
    let result = conn_result(false, 0, "port_unavailable", Some("no listener"), 0);
    let cfg = config("svc-port", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Unhealthy);
}

#[test]
fn failure_connection_failed_is_unhealthy() {
    let result = conn_result(false, 0, "connection_failed", Some("refused"), 0);
    let cfg = config("svc-conn", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Unhealthy);
}

#[test]
fn failure_timeout_is_unhealthy() {
    let result = conn_result(false, 0, "timeout", Some("timed out"), 0);
    let cfg = config("svc-to", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Unhealthy);
}

#[test]
fn failure_unknown_validation_is_unknown() {
    // "success" parses to Success which is neither Auth/Port/Conn/Timeout,
    // so on a failed probe it maps to the catch-all Unknown branch.
    let result = conn_result(false, 0, "success", None, 0);
    let cfg = config("svc-unk", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Unknown);
}

#[test]
fn failure_error_validation_is_unknown() {
    let result = conn_result(false, 0, "error", Some("weird"), 0);
    let cfg = config("svc-err", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.status, HealthStatus::Unknown);
}

#[test]
fn details_carry_error_and_validation_type() {
    let result = conn_result(false, 0, "connection_failed", Some("boom"), 0);
    let cfg = config("svc-d", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.details.service_name, "svc-d");
    assert_eq!(hc.details.error_message.as_deref(), Some("boom"));
    assert_eq!(hc.details.validation_type, "connection_failed");
}

#[test]
fn server_version_extracted_from_server_info() {
    use systemprompt_mcp::services::client::McpProtocolInfo;
    let mut result = conn_result(true, 10, "success", None, 3);
    result.server_info = Some(McpProtocolInfo {
        server_name: "svc".to_string(),
        version: "9.9.9".to_string(),
        protocol_version: "2024".to_string(),
    });
    let cfg = config("svc-v", false);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert_eq!(hc.details.server_version.as_deref(), Some("9.9.9"));
}

#[test]
fn unhealthy_constructor_sets_fields() {
    let cfg = config("svc-x", true);
    let hc = HealthCheckResult::unhealthy(&cfg, "explosion".to_string());
    assert_eq!(hc.status, HealthStatus::Unhealthy);
    assert!(hc.connection_result.is_none());
    assert_eq!(hc.latency_ms, 0);
    assert_eq!(hc.details.service_name, "svc-x");
    assert_eq!(hc.details.error_message.as_deref(), Some("explosion"));
    assert_eq!(hc.details.validation_type, "error");
    assert!(hc.details.requires_auth);
    assert_eq!(hc.details.tools_available, 0);
    assert!(hc.details.server_version.is_none());
}
