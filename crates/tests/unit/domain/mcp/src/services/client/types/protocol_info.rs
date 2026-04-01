//! Unit tests for McpProtocolInfo and ValidationResult

use systemprompt_mcp::services::client::{McpProtocolInfo, ValidationResult};

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
