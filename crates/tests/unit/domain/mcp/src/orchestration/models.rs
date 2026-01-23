//! Unit tests for MCP orchestration models

use chrono::Utc;
use systemprompt_mcp::orchestration::{
    McpServerConnectionInfo, McpServiceState, ServerStatus, SkillLoadingResult,
};

// ============================================================================
// McpServerConnectionInfo Tests
// ============================================================================

#[test]
fn test_mcp_server_connection_info_creation() {
    let info = McpServerConnectionInfo {
        name: "test-server".to_string(),
        display_name: Some("Test Server".to_string()),
        description: Some("A test MCP server".to_string()),
        host: "localhost".to_string(),
        port: 8080,
    };

    assert_eq!(info.name, "test-server");
    assert_eq!(info.display_name, Some("Test Server".to_string()));
    assert_eq!(info.description, Some("A test MCP server".to_string()));
    assert_eq!(info.host, "localhost");
    assert_eq!(info.port, 8080);
}

#[test]
fn test_mcp_server_connection_info_without_optionals() {
    let info = McpServerConnectionInfo {
        name: "minimal".to_string(),
        display_name: None,
        description: None,
        host: "127.0.0.1".to_string(),
        port: 3000,
    };

    assert_eq!(info.name, "minimal");
    assert!(info.display_name.is_none());
    assert!(info.description.is_none());
}

#[test]
fn test_mcp_server_connection_info_clone() {
    let info = McpServerConnectionInfo {
        name: "cloneable".to_string(),
        display_name: Some("Clone Test".to_string()),
        description: Some("Testing clone".to_string()),
        host: "0.0.0.0".to_string(),
        port: 9000,
    };

    let cloned = info.clone();
    assert_eq!(info.name, cloned.name);
    assert_eq!(info.display_name, cloned.display_name);
    assert_eq!(info.description, cloned.description);
    assert_eq!(info.host, cloned.host);
    assert_eq!(info.port, cloned.port);
}

#[test]
fn test_mcp_server_connection_info_debug() {
    let info = McpServerConnectionInfo {
        name: "debug-test".to_string(),
        display_name: None,
        description: None,
        host: "localhost".to_string(),
        port: 5000,
    };

    let debug = format!("{:?}", info);
    assert!(debug.contains("debug-test"));
    assert!(debug.contains("localhost"));
    assert!(debug.contains("5000"));
}

#[test]
fn test_mcp_server_connection_info_port_boundaries() {
    let info_min = McpServerConnectionInfo {
        name: "min-port".to_string(),
        display_name: None,
        description: None,
        host: "localhost".to_string(),
        port: 1,
    };
    assert_eq!(info_min.port, 1);

    let info_max = McpServerConnectionInfo {
        name: "max-port".to_string(),
        display_name: None,
        description: None,
        host: "localhost".to_string(),
        port: 65535,
    };
    assert_eq!(info_max.port, 65535);
}

// ============================================================================
// ServerStatus Tests
// ============================================================================

#[test]
fn test_server_status_running_healthy() {
    let status = ServerStatus {
        name: "healthy-server".to_string(),
        running: true,
        healthy: true,
        tool_count: 10,
        last_check: Some(Utc::now()),
    };

    assert_eq!(status.name, "healthy-server");
    assert!(status.running);
    assert!(status.healthy);
    assert_eq!(status.tool_count, 10);
    assert!(status.last_check.is_some());
}

#[test]
fn test_server_status_stopped() {
    let status = ServerStatus {
        name: "stopped-server".to_string(),
        running: false,
        healthy: false,
        tool_count: 0,
        last_check: None,
    };

    assert!(!status.running);
    assert!(!status.healthy);
    assert_eq!(status.tool_count, 0);
    assert!(status.last_check.is_none());
}

#[test]
fn test_server_status_running_unhealthy() {
    let status = ServerStatus {
        name: "degraded-server".to_string(),
        running: true,
        healthy: false,
        tool_count: 5,
        last_check: Some(Utc::now()),
    };

    assert!(status.running);
    assert!(!status.healthy);
}

#[test]
fn test_server_status_clone() {
    let original = ServerStatus {
        name: "clone-test".to_string(),
        running: true,
        healthy: true,
        tool_count: 3,
        last_check: Some(Utc::now()),
    };

    let cloned = original.clone();
    assert_eq!(original.name, cloned.name);
    assert_eq!(original.running, cloned.running);
    assert_eq!(original.healthy, cloned.healthy);
    assert_eq!(original.tool_count, cloned.tool_count);
}

#[test]
fn test_server_status_debug() {
    let status = ServerStatus {
        name: "debug-server".to_string(),
        running: true,
        healthy: true,
        tool_count: 7,
        last_check: None,
    };

    let debug = format!("{:?}", status);
    assert!(debug.contains("debug-server"));
    assert!(debug.contains("running"));
    assert!(debug.contains("healthy"));
    assert!(debug.contains("7"));
}

#[test]
fn test_server_status_serialization() {
    let status = ServerStatus {
        name: "serialize-test".to_string(),
        running: true,
        healthy: false,
        tool_count: 15,
        last_check: None,
    };

    let json = serde_json::to_string(&status).expect("serialization should succeed");
    assert!(json.contains("serialize-test"));
    assert!(json.contains("true"));
    assert!(json.contains("false"));
    assert!(json.contains("15"));
}

#[test]
fn test_server_status_deserialization() {
    let json = r#"{"name":"deser-test","running":false,"healthy":false,"tool_count":0,"last_check":null}"#;
    let status: ServerStatus = serde_json::from_str(json).expect("deserialization should succeed");

    assert_eq!(status.name, "deser-test");
    assert!(!status.running);
    assert!(!status.healthy);
    assert_eq!(status.tool_count, 0);
    assert!(status.last_check.is_none());
}

#[test]
fn test_server_status_large_tool_count() {
    let status = ServerStatus {
        name: "many-tools".to_string(),
        running: true,
        healthy: true,
        tool_count: 1000,
        last_check: Some(Utc::now()),
    };

    assert_eq!(status.tool_count, 1000);
}

// ============================================================================
// SkillLoadingResult Tests
// ============================================================================

#[test]
fn test_skill_loading_result_success() {
    let result = SkillLoadingResult {
        server_name: "skill-server".to_string(),
        success: true,
        skill_count: 25,
        error_message: None,
        load_time_ms: 150,
    };

    assert_eq!(result.server_name, "skill-server");
    assert!(result.success);
    assert_eq!(result.skill_count, 25);
    assert!(result.error_message.is_none());
    assert_eq!(result.load_time_ms, 150);
}

#[test]
fn test_skill_loading_result_failure() {
    let result = SkillLoadingResult {
        server_name: "failed-server".to_string(),
        success: false,
        skill_count: 0,
        error_message: Some("Connection timeout".to_string()),
        load_time_ms: 5000,
    };

    assert!(!result.success);
    assert_eq!(result.skill_count, 0);
    assert_eq!(result.error_message, Some("Connection timeout".to_string()));
}

#[test]
fn test_skill_loading_result_clone() {
    let original = SkillLoadingResult {
        server_name: "clone-skill".to_string(),
        success: true,
        skill_count: 10,
        error_message: None,
        load_time_ms: 200,
    };

    let cloned = original.clone();
    assert_eq!(original.server_name, cloned.server_name);
    assert_eq!(original.success, cloned.success);
    assert_eq!(original.skill_count, cloned.skill_count);
    assert_eq!(original.error_message, cloned.error_message);
    assert_eq!(original.load_time_ms, cloned.load_time_ms);
}

#[test]
fn test_skill_loading_result_debug() {
    let result = SkillLoadingResult {
        server_name: "debug-skill".to_string(),
        success: true,
        skill_count: 5,
        error_message: None,
        load_time_ms: 100,
    };

    let debug = format!("{:?}", result);
    assert!(debug.contains("debug-skill"));
    assert!(debug.contains("success"));
    assert!(debug.contains("5"));
    assert!(debug.contains("100"));
}

#[test]
fn test_skill_loading_result_zero_load_time() {
    let result = SkillLoadingResult {
        server_name: "instant".to_string(),
        success: true,
        skill_count: 1,
        error_message: None,
        load_time_ms: 0,
    };

    assert_eq!(result.load_time_ms, 0);
}

#[test]
fn test_skill_loading_result_high_load_time() {
    let result = SkillLoadingResult {
        server_name: "slow".to_string(),
        success: true,
        skill_count: 100,
        error_message: None,
        load_time_ms: u64::MAX,
    };

    assert_eq!(result.load_time_ms, u64::MAX);
}

// ============================================================================
// McpServiceState Tests
// ============================================================================

#[test]
fn test_mcp_service_state_running() {
    let state = McpServiceState {
        name: "running-service".to_string(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        status: "running".to_string(),
    };

    assert_eq!(state.name, "running-service");
    assert_eq!(state.host, "127.0.0.1");
    assert_eq!(state.port, 8080);
    assert_eq!(state.status, "running");
}

#[test]
fn test_mcp_service_state_stopped() {
    let state = McpServiceState {
        name: "stopped-service".to_string(),
        host: "localhost".to_string(),
        port: 3000,
        status: "stopped".to_string(),
    };

    assert_eq!(state.status, "stopped");
}

#[test]
fn test_mcp_service_state_starting() {
    let state = McpServiceState {
        name: "starting-service".to_string(),
        host: "0.0.0.0".to_string(),
        port: 5000,
        status: "starting".to_string(),
    };

    assert_eq!(state.status, "starting");
}

#[test]
fn test_mcp_service_state_error() {
    let state = McpServiceState {
        name: "error-service".to_string(),
        host: "192.168.1.1".to_string(),
        port: 9090,
        status: "error".to_string(),
    };

    assert_eq!(state.status, "error");
}

#[test]
fn test_mcp_service_state_clone() {
    let original = McpServiceState {
        name: "clone-service".to_string(),
        host: "localhost".to_string(),
        port: 4000,
        status: "running".to_string(),
    };

    let cloned = original.clone();
    assert_eq!(original.name, cloned.name);
    assert_eq!(original.host, cloned.host);
    assert_eq!(original.port, cloned.port);
    assert_eq!(original.status, cloned.status);
}

#[test]
fn test_mcp_service_state_debug() {
    let state = McpServiceState {
        name: "debug-service".to_string(),
        host: "example.com".to_string(),
        port: 7777,
        status: "healthy".to_string(),
    };

    let debug = format!("{:?}", state);
    assert!(debug.contains("debug-service"));
    assert!(debug.contains("example.com"));
    assert!(debug.contains("7777"));
    assert!(debug.contains("healthy"));
}

#[test]
fn test_mcp_service_state_various_hosts() {
    let ipv4 = McpServiceState {
        name: "ipv4".to_string(),
        host: "192.168.0.1".to_string(),
        port: 80,
        status: "running".to_string(),
    };
    assert_eq!(ipv4.host, "192.168.0.1");

    let ipv6 = McpServiceState {
        name: "ipv6".to_string(),
        host: "::1".to_string(),
        port: 80,
        status: "running".to_string(),
    };
    assert_eq!(ipv6.host, "::1");

    let hostname = McpServiceState {
        name: "hostname".to_string(),
        host: "api.example.com".to_string(),
        port: 443,
        status: "running".to_string(),
    };
    assert_eq!(hostname.host, "api.example.com");
}
