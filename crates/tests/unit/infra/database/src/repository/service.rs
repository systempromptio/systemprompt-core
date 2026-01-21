//! Unit tests for ServiceConfig and CreateServiceInput

use systemprompt_database::{CreateServiceInput, ServiceConfig};

// ============================================================================
// ServiceConfig Tests
// ============================================================================

#[test]
fn test_service_config_creation() {
    let config = ServiceConfig {
        name: "api-server".to_string(),
        module_name: "api".to_string(),
        status: "running".to_string(),
        pid: Some(1234),
        port: 8080,
        binary_mtime: Some(1700000000),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    assert_eq!(config.name, "api-server");
    assert_eq!(config.module_name, "api");
    assert_eq!(config.status, "running");
    assert_eq!(config.pid, Some(1234));
    assert_eq!(config.port, 8080);
}

#[test]
fn test_service_config_without_pid() {
    let config = ServiceConfig {
        name: "stopped-service".to_string(),
        module_name: "mcp".to_string(),
        status: "stopped".to_string(),
        pid: None,
        port: 3000,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    assert!(config.pid.is_none());
    assert_eq!(config.status, "stopped");
}

#[test]
fn test_service_config_debug() {
    let config = ServiceConfig {
        name: "test".to_string(),
        module_name: "test".to_string(),
        status: "running".to_string(),
        pid: Some(1),
        port: 80,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let debug = format!("{:?}", config);
    assert!(debug.contains("ServiceConfig"));
    assert!(debug.contains("test"));
}

#[test]
fn test_service_config_clone() {
    let config = ServiceConfig {
        name: "original".to_string(),
        module_name: "agent".to_string(),
        status: "running".to_string(),
        pid: Some(5678),
        port: 4000,
        binary_mtime: Some(1700000000),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let cloned = config.clone();
    assert_eq!(config.name, cloned.name);
    assert_eq!(config.pid, cloned.pid);
    assert_eq!(config.port, cloned.port);
}

#[test]
fn test_service_config_serialization() {
    let config = ServiceConfig {
        name: "serializable".to_string(),
        module_name: "test".to_string(),
        status: "running".to_string(),
        pid: Some(999),
        port: 9999,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&config).expect("Should serialize");
    assert!(json.contains("\"name\":\"serializable\""));
    assert!(json.contains("\"port\":9999"));
}

#[test]
fn test_service_config_deserialization() {
    let json = r#"{
        "name": "deserialized",
        "module_name": "test",
        "status": "stopped",
        "pid": null,
        "port": 5000,
        "binary_mtime": null,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    }"#;

    let config: ServiceConfig = serde_json::from_str(json).expect("Should deserialize");
    assert_eq!(config.name, "deserialized");
    assert!(config.pid.is_none());
    assert_eq!(config.port, 5000);
}

// ============================================================================
// CreateServiceInput Tests
// ============================================================================

#[test]
fn test_create_service_input_creation() {
    let input = CreateServiceInput {
        name: "new-service",
        module_name: "api",
        status: "starting",
        port: 8080,
        binary_mtime: Some(1700000000),
    };

    assert_eq!(input.name, "new-service");
    assert_eq!(input.module_name, "api");
    assert_eq!(input.status, "starting");
    assert_eq!(input.port, 8080);
    assert_eq!(input.binary_mtime, Some(1700000000));
}

#[test]
fn test_create_service_input_without_mtime() {
    let input = CreateServiceInput {
        name: "simple-service",
        module_name: "mcp",
        status: "stopped",
        port: 3000,
        binary_mtime: None,
    };

    assert!(input.binary_mtime.is_none());
}

#[test]
fn test_create_service_input_debug() {
    let input = CreateServiceInput {
        name: "debug-test",
        module_name: "agent",
        status: "running",
        port: 4000,
        binary_mtime: None,
    };

    let debug = format!("{:?}", input);
    assert!(debug.contains("CreateServiceInput"));
    assert!(debug.contains("debug-test"));
}

#[test]
fn test_create_service_input_port_range() {
    let input = CreateServiceInput {
        name: "high-port",
        module_name: "test",
        status: "running",
        port: 65535,
        binary_mtime: None,
    };

    assert_eq!(input.port, 65535);
}

#[test]
fn test_create_service_input_zero_port() {
    let input = CreateServiceInput {
        name: "zero-port",
        module_name: "test",
        status: "starting",
        port: 0,
        binary_mtime: None,
    };

    assert_eq!(input.port, 0);
}

// ============================================================================
// Service Status Values Tests
// ============================================================================

#[test]
fn test_service_config_status_running() {
    let config = ServiceConfig {
        name: "test".to_string(),
        module_name: "test".to_string(),
        status: "running".to_string(),
        pid: Some(1),
        port: 80,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    assert_eq!(config.status, "running");
}

#[test]
fn test_service_config_status_stopped() {
    let config = ServiceConfig {
        name: "test".to_string(),
        module_name: "test".to_string(),
        status: "stopped".to_string(),
        pid: None,
        port: 80,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    assert_eq!(config.status, "stopped");
    assert!(config.pid.is_none());
}

#[test]
fn test_service_config_status_error() {
    let config = ServiceConfig {
        name: "test".to_string(),
        module_name: "test".to_string(),
        status: "error".to_string(),
        pid: None,
        port: 80,
        binary_mtime: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    assert_eq!(config.status, "error");
}
