use systemprompt_mcp::services::monitoring::health::{HealthCheckDetails, HealthCheckResult, HealthStatus};
use systemprompt_mcp::services::monitoring::status::ServiceStatus;
use systemprompt_mcp::services::schema::{SchemaValidationMode, SchemaValidationReport};
use systemprompt_mcp::services::database::ServiceInfo;
use systemprompt_mcp::services::network::port_manager::{
    MAX_PORT_CLEANUP_ATTEMPTS, PORT_BACKOFF_BASE_MS, POST_KILL_DELAY_MS,
};

#[test]
fn health_status_healthy_as_str() {
    assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
}

#[test]
fn health_status_degraded_as_str() {
    assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
}

#[test]
fn health_status_unhealthy_as_str() {
    assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
}

#[test]
fn health_status_unknown_as_str() {
    assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
}

#[test]
fn health_status_healthy_emoji() {
    let emoji = HealthStatus::Healthy.emoji();
    assert!(!emoji.is_empty());
}

#[test]
fn health_status_unhealthy_emoji() {
    let emoji = HealthStatus::Unhealthy.emoji();
    assert!(!emoji.is_empty());
}

#[test]
fn health_status_equality() {
    assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
    assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    assert_ne!(HealthStatus::Degraded, HealthStatus::Unknown);
}

#[test]
fn health_status_copy() {
    let status = HealthStatus::Healthy;
    let copied = status;
    assert_eq!(status, copied);
}

#[test]
fn health_status_debug() {
    let debug = format!("{:?}", HealthStatus::Degraded);
    assert!(debug.contains("Degraded"));
}

#[test]
fn health_check_details_construction() {
    let details = HealthCheckDetails {
        service_name: "my-service".to_string(),
        tools_available: 5,
        requires_auth: true,
        validation_type: "full".to_string(),
        error_message: None,
        server_version: Some("1.0.0".to_string()),
    };
    assert_eq!(details.service_name, "my-service");
    assert_eq!(details.tools_available, 5);
    assert!(details.requires_auth);
    assert!(details.error_message.is_none());
    assert_eq!(details.server_version.as_deref(), Some("1.0.0"));
}

#[test]
fn health_check_details_with_error() {
    let details = HealthCheckDetails {
        service_name: "failing-svc".to_string(),
        tools_available: 0,
        requires_auth: false,
        validation_type: "port_unavailable".to_string(),
        error_message: Some("Connection refused".to_string()),
        server_version: None,
    };
    assert_eq!(details.error_message.as_deref(), Some("Connection refused"));
    assert!(details.server_version.is_none());
}

#[test]
fn health_check_details_clone() {
    let details = HealthCheckDetails {
        service_name: "clone-test".to_string(),
        tools_available: 3,
        requires_auth: false,
        validation_type: "test".to_string(),
        error_message: None,
        server_version: None,
    };
    let cloned = details.clone();
    assert_eq!(cloned.service_name, "clone-test");
    assert_eq!(cloned.tools_available, 3);
}

#[test]
fn health_check_result_unhealthy_constructor() {
    let config = test_mcp_config("unhealthy-svc");
    let result = HealthCheckResult::unhealthy(&config, "port blocked".to_string());
    assert_eq!(result.status, HealthStatus::Unhealthy);
    assert_eq!(result.latency_ms, 0);
    assert!(result.connection_result.is_none());
    assert_eq!(result.details.service_name, "unhealthy-svc");
    assert_eq!(
        result.details.error_message.as_deref(),
        Some("port blocked")
    );
}

#[test]
fn schema_validation_mode_from_auto_migrate() {
    let mode = SchemaValidationMode::from_string("auto_migrate");
    assert_eq!(mode, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_from_strict() {
    let mode = SchemaValidationMode::from_string("strict");
    assert_eq!(mode, SchemaValidationMode::Strict);
}

#[test]
fn schema_validation_mode_from_skip() {
    let mode = SchemaValidationMode::from_string("skip");
    assert_eq!(mode, SchemaValidationMode::Skip);
}

#[test]
fn schema_validation_mode_from_unknown_defaults_auto() {
    let mode = SchemaValidationMode::from_string("something_else");
    assert_eq!(mode, SchemaValidationMode::AutoMigrate);
}

#[test]
fn schema_validation_mode_case_insensitive() {
    assert_eq!(
        SchemaValidationMode::from_string("STRICT"),
        SchemaValidationMode::Strict
    );
    assert_eq!(
        SchemaValidationMode::from_string("Skip"),
        SchemaValidationMode::Skip
    );
}

#[test]
fn schema_validation_report_new() {
    let report = SchemaValidationReport::new("test-service".to_string());
    assert_eq!(report.service_name, "test-service");
    assert_eq!(report.validated, 0);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn schema_validation_report_merge_accumulates() {
    let mut report = SchemaValidationReport::new("combined".to_string());
    report.validated = 2;
    report.created = 1;

    let other = SchemaValidationReport {
        service_name: "other".to_string(),
        validated: 3,
        created: 2,
        errors: vec!["err1".to_string()],
        warnings: vec!["warn1".to_string()],
    };

    report.merge(other);
    assert_eq!(report.validated, 5);
    assert_eq!(report.created, 3);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn schema_validation_report_merge_empty() {
    let mut report = SchemaValidationReport::new("base".to_string());
    report.validated = 10;

    let empty = SchemaValidationReport::new("empty".to_string());
    report.merge(empty);

    assert_eq!(report.validated, 10);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn schema_validation_report_serde_roundtrip() {
    let report = SchemaValidationReport {
        service_name: "serde-test".to_string(),
        validated: 4,
        created: 1,
        errors: vec!["an error".to_string()],
        warnings: vec!["a warning".to_string()],
    };
    let json = serde_json::to_string(&report).unwrap();
    let deserialized: SchemaValidationReport = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.service_name, "serde-test");
    assert_eq!(deserialized.validated, 4);
    assert_eq!(deserialized.created, 1);
    assert_eq!(deserialized.errors.len(), 1);
    assert_eq!(deserialized.warnings.len(), 1);
}

#[test]
fn service_info_construction() {
    let info = ServiceInfo {
        name: "test-svc".to_string(),
        status: "running".to_string(),
        pid: Some(1234),
        port: 8080,
        binary_mtime: Some(1000),
    };
    assert_eq!(info.name, "test-svc");
    assert_eq!(info.status, "running");
    assert_eq!(info.pid, Some(1234));
    assert_eq!(info.port, 8080);
    assert_eq!(info.binary_mtime, Some(1000));
}

#[test]
fn service_info_without_pid() {
    let info = ServiceInfo {
        name: "no-pid-svc".to_string(),
        status: "stopped".to_string(),
        pid: None,
        port: 9090,
        binary_mtime: None,
    };
    assert!(info.pid.is_none());
    assert!(info.binary_mtime.is_none());
}

#[test]
fn service_info_clone() {
    let info = ServiceInfo {
        name: "clone-svc".to_string(),
        status: "running".to_string(),
        pid: Some(5678),
        port: 3000,
        binary_mtime: Some(9999),
    };
    let cloned = info.clone();
    assert_eq!(cloned.name, info.name);
    assert_eq!(cloned.pid, info.pid);
}

#[test]
fn service_info_debug() {
    let info = ServiceInfo {
        name: "debug-svc".to_string(),
        status: "failed".to_string(),
        pid: None,
        port: 4000,
        binary_mtime: None,
    };
    let debug = format!("{:?}", info);
    assert!(debug.contains("debug-svc"));
    assert!(debug.contains("failed"));
}

#[test]
fn service_status_construction() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(1234),
        health: "healthy".to_string(),
        uptime_seconds: Some(3600),
        tools_count: 10,
        latency_ms: Some(50),
        auth_required: true,
    };
    assert_eq!(status.state, "running");
    assert_eq!(status.pid, Some(1234));
    assert_eq!(status.tools_count, 10);
    assert!(status.auth_required);
}

#[test]
fn service_status_stopped() {
    let status = ServiceStatus {
        state: "stopped".to_string(),
        pid: None,
        health: "unreachable".to_string(),
        uptime_seconds: None,
        tools_count: 0,
        latency_ms: None,
        auth_required: false,
    };
    assert!(status.pid.is_none());
    assert!(status.latency_ms.is_none());
    assert_eq!(status.tools_count, 0);
}

#[test]
fn port_manager_constants_are_reasonable() {
    assert!(MAX_PORT_CLEANUP_ATTEMPTS > 0);
    assert!(MAX_PORT_CLEANUP_ATTEMPTS <= 20);
    assert!(PORT_BACKOFF_BASE_MS > 0);
    assert!(POST_KILL_DELAY_MS > 0);
    assert!(POST_KILL_DELAY_MS <= 5000);
}

fn test_mcp_config(name: &str) -> systemprompt_models::mcp::McpServerConfig {
    systemprompt_models::mcp::McpServerConfig {
        name: name.to_string(),
        server_type: systemprompt_models::mcp::deployment::McpServerType::Internal,
        binary: "test-binary".to_string(),
        enabled: true,
        display_in_web: false,
        port: 8080,
        crate_path: std::path::PathBuf::from("/tmp/test"),
        display_name: name.to_string(),
        description: "Test service".to_string(),
        capabilities: vec![],
        schemas: vec![],
        oauth: systemprompt_models::mcp::deployment::OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: systemprompt_models::auth::JwtAudience::Api,
            client_id: None,
        },
        tools: std::collections::HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "test".to_string(),
        protocol: "sse".to_string(),
        remote_endpoint: String::new(),
    }
}
