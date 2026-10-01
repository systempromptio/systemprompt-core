//! Unit tests for ServiceConfig and CreateServiceInput

use systemprompt_database::{CreateServiceInput, ServiceConfig, ServiceModule, ServiceStatus};
use systemprompt_identifiers::{InstanceId, ServiceName};

fn config(
    name: &str,
    module_name: ServiceModule,
    status: ServiceStatus,
    pid: Option<i32>,
) -> ServiceConfig {
    ServiceConfig {
        instance_id: InstanceId::new("test-instance"),
        name: ServiceName::new(name),
        module_name,
        status,
        pid,
        port: 8080,
        binary_mtime: Some(1700000000),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        heartbeat_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn test_service_config_creation() {
    let config = config(
        "api-server",
        ServiceModule::Agent,
        ServiceStatus::Running,
        Some(1234),
    );

    assert_eq!(config.name.as_str(), "api-server");
    assert_eq!(config.module_name, ServiceModule::Agent);
    assert_eq!(config.status, ServiceStatus::Running);
    assert_eq!(config.pid, Some(1234));
    assert_eq!(config.port, 8080);
}

#[test]
fn test_service_config_without_pid() {
    let config = config(
        "stopped-service",
        ServiceModule::Mcp,
        ServiceStatus::Stopped,
        None,
    );

    assert!(config.pid.is_none());
    assert_eq!(config.status, ServiceStatus::Stopped);
}

#[test]
fn test_service_config_serializes_the_stored_strings() {
    let config = config(
        "serializable",
        ServiceModule::Mcp,
        ServiceStatus::Running,
        Some(999),
    );

    let json = serde_json::to_string(&config).expect("Should serialize");
    assert!(json.contains("\"name\":\"serializable\""));
    assert!(json.contains("\"module_name\":\"mcp\""));
    assert!(json.contains("\"status\":\"running\""));
}

#[test]
fn test_service_status_round_trips_every_stored_string() {
    for status in [
        ServiceStatus::Starting,
        ServiceStatus::Running,
        ServiceStatus::Stopping,
        ServiceStatus::Stopped,
        ServiceStatus::Error,
    ] {
        assert_eq!(status.as_str().parse::<ServiceStatus>(), Ok(status));
    }
    assert!("crashed".parse::<ServiceStatus>().is_err());
    assert!("".parse::<ServiceStatus>().is_err());
}

#[test]
fn test_service_module_round_trips_every_stored_string() {
    for module in [ServiceModule::Mcp, ServiceModule::Agent] {
        assert_eq!(module.as_str().parse::<ServiceModule>(), Ok(module));
    }
    assert!("api".parse::<ServiceModule>().is_err());
}

#[test]
fn test_create_service_input_creation() {
    let name = ServiceName::new("new-service");
    let input = CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Agent,
        status: ServiceStatus::Starting,
        port: 8080,
        binary_mtime: Some(1700000000),
    };

    assert_eq!(input.name.as_str(), "new-service");
    assert_eq!(input.module_name, ServiceModule::Agent);
    assert_eq!(input.status, ServiceStatus::Starting);
    assert_eq!(input.port, 8080);
    assert_eq!(input.binary_mtime, Some(1700000000));
}

#[test]
fn test_create_service_input_debug() {
    let name = ServiceName::new("debug-test");
    let input = CreateServiceInput {
        name: &name,
        module_name: ServiceModule::Agent,
        status: ServiceStatus::Running,
        port: 4000,
        binary_mtime: None,
    };

    let debug = format!("{:?}", input);
    assert!(debug.contains("CreateServiceInput"));
    assert!(debug.contains("debug-test"));
}
