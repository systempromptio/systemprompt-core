//! Unit tests for HealthCheckResult and HealthCheckDetails

use systemprompt_mcp::services::monitoring::health::{
    HealthCheckDetails, HealthCheckResult, HealthStatus,
};

// ============================================================================
// HealthCheckDetails Tests
// ============================================================================

#[test]
fn test_health_check_details_new() {
    let details = HealthCheckDetails {
        service_name: "test-service".to_string(),
        tools_available: 5,
        requires_auth: false,
        validation_type: "mcp_validated".to_string(),
        error_message: None,
        server_version: Some("1.0.0".to_string()),
    };

    assert_eq!(details.service_name, "test-service");
    assert_eq!(details.tools_available, 5);
    assert!(!details.requires_auth);
    assert_eq!(details.validation_type, "mcp_validated");
    assert!(details.error_message.is_none());
    assert_eq!(details.server_version, Some("1.0.0".to_string()));
}

#[test]
fn test_health_check_details_with_error() {
    let details = HealthCheckDetails {
        service_name: "failing-service".to_string(),
        tools_available: 0,
        requires_auth: true,
        validation_type: "connection_failed".to_string(),
        error_message: Some("Connection refused".to_string()),
        server_version: None,
    };

    assert_eq!(details.service_name, "failing-service");
    assert_eq!(details.tools_available, 0);
    assert!(details.requires_auth);
    assert_eq!(details.validation_type, "connection_failed");
    assert_eq!(details.error_message, Some("Connection refused".to_string()));
    assert!(details.server_version.is_none());
}

#[test]
fn test_health_check_details_auth_required() {
    let details = HealthCheckDetails {
        service_name: "oauth-service".to_string(),
        tools_available: 0,
        requires_auth: true,
        validation_type: "auth_required".to_string(),
        error_message: None,
        server_version: Some("2.0.0".to_string()),
    };

    assert!(details.requires_auth);
    assert_eq!(details.validation_type, "auth_required");
}

#[test]
fn test_health_check_details_clone() {
    let details = HealthCheckDetails {
        service_name: "test-service".to_string(),
        tools_available: 10,
        requires_auth: false,
        validation_type: "success".to_string(),
        error_message: None,
        server_version: Some("1.0.0".to_string()),
    };

    let cloned = details.clone();
    assert_eq!(details.service_name, cloned.service_name);
    assert_eq!(details.tools_available, cloned.tools_available);
    assert_eq!(details.server_version, cloned.server_version);
}

#[test]
fn test_health_check_details_debug() {
    let details = HealthCheckDetails {
        service_name: "debug-service".to_string(),
        tools_available: 3,
        requires_auth: true,
        validation_type: "success".to_string(),
        error_message: None,
        server_version: None,
    };

    let debug_str = format!("{:?}", details);
    assert!(debug_str.contains("HealthCheckDetails"));
    assert!(debug_str.contains("debug-service"));
}

// ============================================================================
// HealthCheckResult Tests
// ============================================================================

#[test]
fn test_health_check_result_healthy() {
    let details = HealthCheckDetails {
        service_name: "healthy-service".to_string(),
        tools_available: 5,
        requires_auth: false,
        validation_type: "mcp_validated".to_string(),
        error_message: None,
        server_version: Some("1.0.0".to_string()),
    };

    let result = HealthCheckResult {
        status: HealthStatus::Healthy,
        connection_result: None,
        latency_ms: 50,
        details,
    };

    assert_eq!(result.status, HealthStatus::Healthy);
    assert_eq!(result.latency_ms, 50);
    assert!(result.connection_result.is_none());
}

#[test]
fn test_health_check_result_degraded() {
    let details = HealthCheckDetails {
        service_name: "slow-service".to_string(),
        tools_available: 3,
        requires_auth: false,
        validation_type: "mcp_validated".to_string(),
        error_message: None,
        server_version: Some("1.0.0".to_string()),
    };

    let result = HealthCheckResult {
        status: HealthStatus::Degraded,
        connection_result: None,
        latency_ms: 1500,
        details,
    };

    assert_eq!(result.status, HealthStatus::Degraded);
    assert!(result.latency_ms > 1000);
}

#[test]
fn test_health_check_result_unhealthy() {
    let details = HealthCheckDetails {
        service_name: "dead-service".to_string(),
        tools_available: 0,
        requires_auth: false,
        validation_type: "connection_failed".to_string(),
        error_message: Some("Connection refused".to_string()),
        server_version: None,
    };

    let result = HealthCheckResult {
        status: HealthStatus::Unhealthy,
        connection_result: None,
        latency_ms: 0,
        details,
    };

    assert_eq!(result.status, HealthStatus::Unhealthy);
    assert!(result.details.error_message.is_some());
}

#[test]
fn test_health_check_result_unknown() {
    let details = HealthCheckDetails {
        service_name: "mystery-service".to_string(),
        tools_available: 0,
        requires_auth: false,
        validation_type: "error".to_string(),
        error_message: Some("Unknown error".to_string()),
        server_version: None,
    };

    let result = HealthCheckResult {
        status: HealthStatus::Unknown,
        connection_result: None,
        latency_ms: 0,
        details,
    };

    assert_eq!(result.status, HealthStatus::Unknown);
}

#[test]
fn test_health_check_result_clone() {
    let details = HealthCheckDetails {
        service_name: "clone-service".to_string(),
        tools_available: 2,
        requires_auth: true,
        validation_type: "auth_required".to_string(),
        error_message: None,
        server_version: Some("1.5.0".to_string()),
    };

    let result = HealthCheckResult {
        status: HealthStatus::Healthy,
        connection_result: None,
        latency_ms: 100,
        details,
    };

    let cloned = result.clone();
    assert_eq!(result.status, cloned.status);
    assert_eq!(result.latency_ms, cloned.latency_ms);
    assert_eq!(result.details.service_name, cloned.details.service_name);
}

#[test]
fn test_health_check_result_debug() {
    let details = HealthCheckDetails {
        service_name: "debug-result-service".to_string(),
        tools_available: 1,
        requires_auth: false,
        validation_type: "success".to_string(),
        error_message: None,
        server_version: None,
    };

    let result = HealthCheckResult {
        status: HealthStatus::Healthy,
        connection_result: None,
        latency_ms: 75,
        details,
    };

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("HealthCheckResult"));
    assert!(debug_str.contains("Healthy"));
}
