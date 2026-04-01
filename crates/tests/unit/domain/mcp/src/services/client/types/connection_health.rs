//! Unit tests for McpConnectionResult health checks

use systemprompt_mcp::services::client::{McpConnectionResult, McpProtocolInfo};

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
