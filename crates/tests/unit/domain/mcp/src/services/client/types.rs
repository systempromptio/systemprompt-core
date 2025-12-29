//! Unit tests for MCP client types

use systemprompt_core_mcp::services::client::{McpConnectionResult, McpProtocolInfo, ValidationResult};

// ============================================================================
// McpProtocolInfo Tests
// ============================================================================

#[test]
fn test_mcp_protocol_info_creation() {
    let info = McpProtocolInfo {
        server_name: "test-server".to_string(),
        version: "1.0.0".to_string(),
        protocol_version: "2024-11-05".to_string(),
    };

    assert_eq!(info.server_name, "test-server");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.protocol_version, "2024-11-05");
}

#[test]
fn test_mcp_protocol_info_clone() {
    let info = McpProtocolInfo {
        server_name: "test-server".to_string(),
        version: "1.0.0".to_string(),
        protocol_version: "2024-11-05".to_string(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.server_name, info.server_name);
    assert_eq!(cloned.version, info.version);
    assert_eq!(cloned.protocol_version, info.protocol_version);
}

#[test]
fn test_mcp_protocol_info_debug() {
    let info = McpProtocolInfo {
        server_name: "test-server".to_string(),
        version: "1.0.0".to_string(),
        protocol_version: "2024-11-05".to_string(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("McpProtocolInfo"));
    assert!(debug_str.contains("test-server"));
}

#[test]
fn test_mcp_protocol_info_serialize() {
    let info = McpProtocolInfo {
        server_name: "test-server".to_string(),
        version: "1.0.0".to_string(),
        protocol_version: "2024-11-05".to_string(),
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("test-server"));
    assert!(json.contains("1.0.0"));
}

#[test]
fn test_mcp_protocol_info_deserialize() {
    let json = r#"{"server_name":"test-server","version":"1.0.0","protocol_version":"2024-11-05"}"#;
    let info: McpProtocolInfo = serde_json::from_str(json).unwrap();

    assert_eq!(info.server_name, "test-server");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.protocol_version, "2024-11-05");
}

// ============================================================================
// ValidationResult Tests
// ============================================================================

#[test]
fn test_validation_result_success() {
    let result = ValidationResult {
        success: true,
        error_message: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    assert!(result.success);
    assert!(result.error_message.is_none());
    assert_eq!(result.tools_count, 5);
    assert_eq!(result.validation_type, "mcp_validated");
}

#[test]
fn test_validation_result_failure() {
    let result = ValidationResult {
        success: false,
        error_message: Some("Connection failed".to_string()),
        tools_count: 0,
        validation_type: "connection_failed".to_string(),
    };

    assert!(!result.success);
    assert_eq!(result.error_message, Some("Connection failed".to_string()));
    assert_eq!(result.tools_count, 0);
    assert_eq!(result.validation_type, "connection_failed");
}

#[test]
fn test_validation_result_clone() {
    let result = ValidationResult {
        success: true,
        error_message: None,
        tools_count: 3,
        validation_type: "mcp_validated".to_string(),
    };

    let cloned = result.clone();
    assert_eq!(cloned.success, result.success);
    assert_eq!(cloned.tools_count, result.tools_count);
}

#[test]
fn test_validation_result_debug() {
    let result = ValidationResult {
        success: true,
        error_message: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("ValidationResult"));
}

// ============================================================================
// McpConnectionResult Tests
// ============================================================================

#[test]
fn test_mcp_connection_result_healthy() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: Some(McpProtocolInfo {
            server_name: "test-server".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: "2024-11-05".to_string(),
        }),
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    assert!(result.is_healthy());
}

#[test]
fn test_mcp_connection_result_unhealthy_slow() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 2500,
        server_info: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    assert!(!result.is_healthy());
}

#[test]
fn test_mcp_connection_result_unhealthy_failed() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: Some("Connection failed".to_string()),
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "connection_failed".to_string(),
    };

    assert!(!result.is_healthy());
}

#[test]
fn test_mcp_connection_result_health_status_healthy() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    assert_eq!(result.health_status(), "healthy");
}

#[test]
fn test_mcp_connection_result_health_status_slow() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 1500,
        server_info: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    assert_eq!(result.health_status(), "slow");
}

#[test]
fn test_mcp_connection_result_health_status_auth_required() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "auth_required".to_string(),
    };

    assert_eq!(result.health_status(), "auth_required");
}

#[test]
fn test_mcp_connection_result_health_status_no_tools() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "no_tools".to_string(),
    };

    assert_eq!(result.health_status(), "auth_required");
}

#[test]
fn test_mcp_connection_result_health_status_unhealthy_variants() {
    let variants = [
        "tools_request_failed",
        "connection_failed",
        "port_unavailable",
        "timeout",
    ];

    for variant in variants {
        let result = McpConnectionResult {
            service_name: "test-service".to_string(),
            success: false,
            error_message: None,
            connection_time_ms: 100,
            server_info: None,
            tools_count: 0,
            validation_type: variant.to_string(),
        };

        assert_eq!(result.health_status(), "unhealthy");
    }
}

#[test]
fn test_mcp_connection_result_health_status_unknown() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "some_unknown_type".to_string(),
    };

    assert_eq!(result.health_status(), "unknown");
}

#[test]
fn test_mcp_connection_result_status_description_mcp_validated() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("MCP validated"));
    assert!(description.contains("5 tools"));
}

#[test]
fn test_mcp_connection_result_status_description_auth_required() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "auth_required".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("OAuth authentication required"));
}

#[test]
fn test_mcp_connection_result_status_description_no_tools() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "no_tools".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("no tools returned"));
}

#[test]
fn test_mcp_connection_result_status_description_tools_request_failed() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: Some("RPC error".to_string()),
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "tools_request_failed".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("Tools request failed"));
    assert!(description.contains("RPC error"));
}

#[test]
fn test_mcp_connection_result_status_description_connection_failed() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: Some("Network error".to_string()),
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "connection_failed".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("Connection failed"));
    assert!(description.contains("Network error"));
}

#[test]
fn test_mcp_connection_result_status_description_port_unavailable() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "port_unavailable".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("Port not responding"));
}

#[test]
fn test_mcp_connection_result_status_description_timeout() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "timeout".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("Connection timeout"));
}

#[test]
fn test_mcp_connection_result_status_description_unknown() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "unknown_type".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("Unknown validation result"));
}

#[test]
fn test_mcp_connection_result_clone() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: Some(McpProtocolInfo {
            server_name: "test-server".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: "2024-11-05".to_string(),
        }),
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    let cloned = result.clone();
    assert_eq!(cloned.service_name, result.service_name);
    assert_eq!(cloned.success, result.success);
    assert_eq!(cloned.tools_count, result.tools_count);
}

#[test]
fn test_mcp_connection_result_serialize() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: true,
        error_message: None,
        connection_time_ms: 100,
        server_info: None,
        tools_count: 5,
        validation_type: "mcp_validated".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("test-service"));
    assert!(json.contains("mcp_validated"));
}

#[test]
fn test_mcp_connection_result_deserialize() {
    let json = r#"{"service_name":"test-service","success":true,"error_message":null,"connection_time_ms":100,"server_info":null,"tools_count":5,"validation_type":"mcp_validated"}"#;
    let result: McpConnectionResult = serde_json::from_str(json).unwrap();

    assert_eq!(result.service_name, "test-service");
    assert!(result.success);
    assert_eq!(result.tools_count, 5);
}

#[test]
fn test_mcp_connection_result_status_description_no_error_message() {
    let result = McpConnectionResult {
        service_name: "test-service".to_string(),
        success: false,
        error_message: Some(String::new()),
        connection_time_ms: 100,
        server_info: None,
        tools_count: 0,
        validation_type: "tools_request_failed".to_string(),
    };

    let description = result.status_description();
    assert!(description.contains("[no error message]"));
}
