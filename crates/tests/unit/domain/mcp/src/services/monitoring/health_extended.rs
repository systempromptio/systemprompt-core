//! Extended coverage for monitoring/health.rs: `HealthCheckDetails` fields,
//! `HealthCheckResult` clone/debug, and timeout/unknown status transitions.

use std::path::PathBuf;
use systemprompt_mcp::services::client::{McpConnectionResult, McpProtocolInfo};
use systemprompt_mcp::services::monitoring::health::{
    HealthCheckDetails, HealthCheckResult, HealthStatus,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn config(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: false,
        port: 0,
        crate_path: PathBuf::from("."),
        display_name: name.to_owned(),
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

fn conn(
    success: bool,
    time_ms: u32,
    vtype: &str,
    tools: usize,
    info: Option<McpProtocolInfo>,
) -> McpConnectionResult {
    McpConnectionResult {
        service_name: "test".to_owned(),
        success,
        error_message: if success {
            None
        } else {
            Some("err".to_owned())
        },
        connection_time_ms: time_ms,
        server_info: info,
        tools_count: tools,
        validation_type: vtype.to_owned(),
    }
}

#[test]
fn health_check_result_clone_and_debug() {
    let cfg = config("c1");
    let hc = HealthCheckResult::unhealthy(&cfg, "some error".to_owned());
    let cloned = hc.clone();
    assert!(matches!(cloned.status, HealthStatus::Unhealthy));
    let dbg = format!("{cloned:?}");
    assert!(dbg.contains("Unhealthy"));
}

#[test]
fn health_check_details_fields_populated_from_connection_result() {
    let cfg = config("details-test");
    let proto = McpProtocolInfo {
        server_name: "details-test".to_owned(),
        version: "2.5.0".to_owned(),
        protocol_version: "2025-01-01".to_owned(),
    };
    let result = conn(true, 50, "mcp_validated", 5, Some(proto));
    let hc = HealthCheckResult::from_connection_result(result, &cfg);

    assert_eq!(hc.details.service_name, "details-test");
    assert_eq!(hc.details.tools_available, 5);
    assert!(!hc.details.requires_auth);
    assert_eq!(hc.details.server_version.as_deref(), Some("2.5.0"));
    assert!(hc.details.error_message.is_none());
    assert_eq!(hc.latency_ms, 50);
}

#[test]
fn health_check_details_no_server_info_is_none() {
    let cfg = config("no-info");
    let result = conn(true, 200, "mcp_validated", 2, None);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert!(hc.details.server_version.is_none());
}

#[test]
fn health_check_result_connection_result_is_some_on_success() {
    let cfg = config("conn-some");
    let result = conn(true, 100, "mcp_validated", 1, None);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    let cr = hc.connection_result.expect("connection result on success");
    assert!(cr.success);
    assert_eq!(cr.tools_count, 1);
}

#[test]
fn health_check_result_connection_result_is_none_on_unhealthy_ctor() {
    let cfg = config("conn-none");
    let hc = HealthCheckResult::unhealthy(&cfg, "timeout".to_owned());
    assert!(hc.connection_result.is_none());
    assert_eq!(hc.latency_ms, 0);
}

#[test]
fn health_check_details_auth_required_flag_propagated() {
    let mut cfg = config("auth-flag");
    cfg.oauth.required = true;

    let result = conn(false, 0, "auth_required", 0, None);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert!(hc.details.requires_auth);
    assert!(matches!(hc.status, HealthStatus::Healthy));
}

#[test]
fn health_check_timeout_validation_type_is_unhealthy() {
    let cfg = config("timeout-check");
    let result = conn(false, 5000, "timeout", 0, None);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert!(matches!(hc.status, HealthStatus::Unhealthy));
    assert!(hc.details.validation_type.contains("timeout"));
}

#[test]
fn health_check_unknown_validation_type_is_unknown() {
    let cfg = config("unk-type");
    let result = conn(false, 0, "something_novel", 0, None);
    let hc = HealthCheckResult::from_connection_result(result, &cfg);
    assert!(matches!(hc.status, HealthStatus::Unknown));
}

#[test]
fn health_check_details_error_message_from_connection_result() {
    let cfg = config("err-msg");
    let mut r = conn(false, 0, "connection_failed", 0, None);
    r.error_message = Some("connection refused".to_owned());
    let hc = HealthCheckResult::from_connection_result(r, &cfg);
    assert_eq!(
        hc.details.error_message.as_deref(),
        Some("connection refused")
    );
}

#[test]
fn health_check_details_clone_and_debug() {
    let d = HealthCheckDetails {
        service_name: "svc".to_owned(),
        tools_available: 3,
        requires_auth: true,
        validation_type: "mcp_validated".to_owned(),
        error_message: None,
        server_version: Some("1.0".to_owned()),
    };
    let cloned = d.clone();
    assert_eq!(cloned.service_name, d.service_name);
    let dbg = format!("{d:?}");
    assert!(dbg.contains("HealthCheckDetails"));
}

#[test]
fn health_status_equality() {
    assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
    assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    assert_ne!(HealthStatus::Degraded, HealthStatus::Unknown);
}

#[test]
fn health_status_copy() {
    let a = HealthStatus::Degraded;
    let b = a;
    assert_eq!(a, b);
}
