//! Unit tests for McpError types

use systemprompt_mcp::McpError;

// ============================================================================
// McpError Display Tests
// ============================================================================

#[test]
fn test_mcp_error_server_not_found_display() {
    let error = McpError::ServerNotFound("test-server".to_string());
    let display = error.to_string();
    assert!(display.contains("test-server"));
    assert!(display.contains("not found"));
}

#[test]
fn test_mcp_error_connection_failed_display() {
    let error = McpError::ConnectionFailed {
        server: "my-server".to_string(),
        message: "connection refused".to_string(),
    };
    let display = error.to_string();
    assert!(display.contains("my-server"));
    assert!(display.contains("connection refused"));
}

#[test]
fn test_mcp_error_tool_execution_failed_display() {
    let error = McpError::ToolExecutionFailed("tool invocation timeout".to_string());
    let display = error.to_string();
    assert!(display.contains("tool invocation timeout"));
}

#[test]
fn test_mcp_error_schema_validation_display() {
    let error = McpError::SchemaValidation("invalid schema format".to_string());
    let display = error.to_string();
    assert!(display.contains("invalid schema format"));
}

#[test]
fn test_mcp_error_registry_validation_display() {
    let error = McpError::RegistryValidation("missing required field".to_string());
    let display = error.to_string();
    assert!(display.contains("missing required field"));
}

#[test]
fn test_mcp_error_process_spawn_display() {
    let error = McpError::ProcessSpawn {
        server: "worker-server".to_string(),
        message: "binary not found".to_string(),
    };
    let display = error.to_string();
    assert!(display.contains("worker-server"));
    assert!(display.contains("binary not found"));
}

#[test]
fn test_mcp_error_port_unavailable_display() {
    let error = McpError::PortUnavailable {
        port: 8080,
        message: "address already in use".to_string(),
    };
    let display = error.to_string();
    assert!(display.contains("8080"));
    assert!(display.contains("address already in use"));
}

#[test]
fn test_mcp_error_configuration_display() {
    let error = McpError::Configuration("invalid config value".to_string());
    let display = error.to_string();
    assert!(display.contains("invalid config value"));
}

#[test]
fn test_mcp_error_auth_required_display() {
    let error = McpError::AuthRequired("github-api".to_string());
    let display = error.to_string();
    assert!(display.contains("github-api"));
    assert!(display.contains("Authentication") || display.contains("required"));
}

#[test]
fn test_mcp_error_internal_display() {
    let error = McpError::Internal("unexpected internal error".to_string());
    let display = error.to_string();
    assert!(display.contains("unexpected internal error"));
}

// ============================================================================
// McpError Debug Tests
// ============================================================================

#[test]
fn test_mcp_error_server_not_found_debug() {
    let error = McpError::ServerNotFound("debug-server".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("ServerNotFound"));
    assert!(debug.contains("debug-server"));
}

#[test]
fn test_mcp_error_connection_failed_debug() {
    let error = McpError::ConnectionFailed {
        server: "test".to_string(),
        message: "timeout".to_string(),
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("ConnectionFailed"));
}

#[test]
fn test_mcp_error_tool_execution_failed_debug() {
    let error = McpError::ToolExecutionFailed("error".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("ToolExecutionFailed"));
}

#[test]
fn test_mcp_error_schema_validation_debug() {
    let error = McpError::SchemaValidation("bad schema".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("SchemaValidation"));
}

#[test]
fn test_mcp_error_registry_validation_debug() {
    let error = McpError::RegistryValidation("invalid".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("RegistryValidation"));
}

#[test]
fn test_mcp_error_process_spawn_debug() {
    let error = McpError::ProcessSpawn {
        server: "s".to_string(),
        message: "m".to_string(),
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("ProcessSpawn"));
}

#[test]
fn test_mcp_error_port_unavailable_debug() {
    let error = McpError::PortUnavailable {
        port: 3000,
        message: "in use".to_string(),
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("PortUnavailable"));
}

#[test]
fn test_mcp_error_configuration_debug() {
    let error = McpError::Configuration("bad config".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("Configuration"));
}

#[test]
fn test_mcp_error_auth_required_debug() {
    let error = McpError::AuthRequired("service".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("AuthRequired"));
}

#[test]
fn test_mcp_error_internal_debug() {
    let error = McpError::Internal("internal".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("Internal"));
}

// ============================================================================
// McpError From Implementations Tests
// ============================================================================

#[test]
fn test_mcp_error_from_serde_json() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let mcp_error: McpError = json_error.into();
    let display = mcp_error.to_string();
    assert!(!display.is_empty());
}

// ============================================================================
// McpResult Type Tests
// ============================================================================

#[test]
fn test_mcp_result_ok() {
    let result: Result<i32, McpError> = Ok(42);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_mcp_result_err() {
    let result: Result<i32, McpError> = Err(McpError::ServerNotFound("test".to_string()));
    assert!(result.is_err());
}

#[test]
fn test_mcp_result_map() {
    let result: Result<i32, McpError> = Ok(10);
    let mapped = result.map(|x| x * 2);
    assert_eq!(mapped.unwrap(), 20);
}

#[test]
fn test_mcp_result_and_then() {
    let result: Result<i32, McpError> = Ok(5);
    let chained: Result<i32, McpError> = result.and_then(|x| Ok(x + 1));
    assert_eq!(chained.unwrap(), 6);
}

// ============================================================================
// McpError Edge Cases
// ============================================================================

#[test]
fn test_mcp_error_empty_server_name() {
    let error = McpError::ServerNotFound(String::new());
    let display = error.to_string();
    assert!(display.contains("not found"));
}

#[test]
fn test_mcp_error_empty_message() {
    let error = McpError::ConnectionFailed {
        server: "server".to_string(),
        message: String::new(),
    };
    let display = error.to_string();
    assert!(display.contains("server"));
}

#[test]
fn test_mcp_error_port_zero() {
    let error = McpError::PortUnavailable {
        port: 0,
        message: "invalid port".to_string(),
    };
    let display = error.to_string();
    assert!(display.contains("0"));
}

#[test]
fn test_mcp_error_port_max() {
    let error = McpError::PortUnavailable {
        port: u16::MAX,
        message: "out of range".to_string(),
    };
    let display = error.to_string();
    assert!(display.contains("65535"));
}

#[test]
fn test_mcp_error_unicode_message() {
    let error = McpError::Configuration("配置错误: invalid 设置".to_string());
    let display = error.to_string();
    assert!(display.contains("配置错误"));
    assert!(display.contains("设置"));
}

#[test]
fn test_mcp_error_long_message() {
    let long_message = "x".repeat(10000);
    let error = McpError::Internal(long_message.clone());
    let display = error.to_string();
    assert!(display.contains(&long_message));
}
