//! Unit tests for MCPService model

use chrono::Utc;
use systemprompt_core_mcp::models::MCPService;
use uuid::Uuid;

fn create_test_service(status: &str, health: &str) -> MCPService {
    MCPService {
        id: Uuid::new_v4(),
        name: "test-service".to_string(),
        module: "test-module".to_string(),
        port: 8080,
        pid: Some(1234),
        status: status.to_string(),
        health: health.to_string(),
        restart_count: 0,
        last_health_check: Some(Utc::now()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// ============================================================================
// MCPService is_running Tests
// ============================================================================

#[test]
fn test_mcp_service_is_running_true() {
    let service = create_test_service("running", "healthy");
    assert!(service.is_running());
}

#[test]
fn test_mcp_service_is_running_false_stopped() {
    let service = create_test_service("stopped", "healthy");
    assert!(!service.is_running());
}

#[test]
fn test_mcp_service_is_running_false_error() {
    let service = create_test_service("error", "healthy");
    assert!(!service.is_running());
}

#[test]
fn test_mcp_service_is_running_false_starting() {
    let service = create_test_service("starting", "healthy");
    assert!(!service.is_running());
}

// ============================================================================
// MCPService is_healthy Tests
// ============================================================================

#[test]
fn test_mcp_service_is_healthy_true() {
    let service = create_test_service("running", "healthy");
    assert!(service.is_healthy());
}

#[test]
fn test_mcp_service_is_healthy_false_unhealthy() {
    let service = create_test_service("running", "unhealthy");
    assert!(!service.is_healthy());
}

#[test]
fn test_mcp_service_is_healthy_false_degraded() {
    let service = create_test_service("running", "degraded");
    assert!(!service.is_healthy());
}

#[test]
fn test_mcp_service_is_healthy_false_unknown() {
    let service = create_test_service("running", "unknown");
    assert!(!service.is_healthy());
}

// ============================================================================
// MCPService Field Access Tests
// ============================================================================

#[test]
fn test_mcp_service_fields() {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let service = MCPService {
        id,
        name: "my-service".to_string(),
        module: "my-module".to_string(),
        port: 9090,
        pid: Some(5678),
        status: "running".to_string(),
        health: "healthy".to_string(),
        restart_count: 3,
        last_health_check: Some(now),
        created_at: now,
        updated_at: now,
    };

    assert_eq!(service.id, id);
    assert_eq!(service.name, "my-service");
    assert_eq!(service.module, "my-module");
    assert_eq!(service.port, 9090);
    assert_eq!(service.pid, Some(5678));
    assert_eq!(service.status, "running");
    assert_eq!(service.health, "healthy");
    assert_eq!(service.restart_count, 3);
}

#[test]
fn test_mcp_service_no_pid() {
    let mut service = create_test_service("stopped", "unknown");
    service.pid = None;
    assert!(service.pid.is_none());
}

#[test]
fn test_mcp_service_no_last_health_check() {
    let mut service = create_test_service("starting", "unknown");
    service.last_health_check = None;
    assert!(service.last_health_check.is_none());
}

// ============================================================================
// MCPService Clone Tests
// ============================================================================

#[test]
fn test_mcp_service_clone() {
    let service = create_test_service("running", "healthy");
    let cloned = service.clone();

    assert_eq!(service.id, cloned.id);
    assert_eq!(service.name, cloned.name);
    assert_eq!(service.status, cloned.status);
    assert_eq!(service.health, cloned.health);
}

// ============================================================================
// MCPService Debug Tests
// ============================================================================

#[test]
fn test_mcp_service_debug() {
    let service = create_test_service("running", "healthy");
    let debug_str = format!("{:?}", service);

    assert!(debug_str.contains("MCPService"));
    assert!(debug_str.contains("test-service"));
    assert!(debug_str.contains("running"));
    assert!(debug_str.contains("healthy"));
}

// ============================================================================
// MCPService Serialization Tests
// ============================================================================

#[test]
fn test_mcp_service_serialize() {
    let service = create_test_service("running", "healthy");
    let json = serde_json::to_string(&service).unwrap();

    assert!(json.contains("test-service"));
    assert!(json.contains("test-module"));
    assert!(json.contains("8080"));
    assert!(json.contains("running"));
    assert!(json.contains("healthy"));
}

#[test]
fn test_mcp_service_deserialize() {
    let service = create_test_service("running", "healthy");
    let json = serde_json::to_string(&service).unwrap();
    let deserialized: MCPService = serde_json::from_str(&json).unwrap();

    assert_eq!(service.name, deserialized.name);
    assert_eq!(service.status, deserialized.status);
    assert_eq!(service.health, deserialized.health);
    assert_eq!(service.port, deserialized.port);
}
