//! Unit tests for McpConnectionResult status_description

use systemprompt_mcp::services::client::McpConnectionResult;

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
