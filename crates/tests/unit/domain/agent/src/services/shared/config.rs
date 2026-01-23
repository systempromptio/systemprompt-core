//! Unit tests for service configuration types
//!
//! Tests cover:
//! - ServiceConfiguration validation and defaults
//! - RuntimeConfiguration builder pattern
//! - AgentServiceConfig validation
//! - ConnectionConfiguration methods

use std::time::Duration;
use systemprompt_agent::services::shared::config::{
    AgentServiceConfig, ConfigValidation, ConnectionConfiguration, RuntimeConfiguration,
    RuntimeConfigurationBuilder, ServiceConfiguration,
};
use systemprompt_identifiers::AgentId;

// ============================================================================
// ServiceConfiguration Tests
// ============================================================================

#[test]
fn test_service_configuration_default() {
    let config = ServiceConfiguration::default();

    assert!(config.enabled);
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.retry_attempts, 3);
    assert_eq!(config.retry_delay_milliseconds, 500);
    assert_eq!(config.max_connections, 10);
}

#[test]
fn test_service_configuration_timeout() {
    let config = ServiceConfiguration {
        enabled: true,
        timeout_seconds: 60,
        retry_attempts: 3,
        retry_delay_milliseconds: 500,
        max_connections: 10,
    };

    assert_eq!(config.timeout(), Duration::from_secs(60));
}

#[test]
fn test_service_configuration_retry_delay() {
    let config = ServiceConfiguration {
        enabled: true,
        timeout_seconds: 30,
        retry_attempts: 3,
        retry_delay_milliseconds: 1000,
        max_connections: 10,
    };

    assert_eq!(config.retry_delay(), Duration::from_millis(1000));
}

#[test]
fn test_service_configuration_validate_success() {
    let config = ServiceConfiguration::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_service_configuration_validate_zero_retry_attempts() {
    let config = ServiceConfiguration {
        enabled: true,
        timeout_seconds: 30,
        retry_attempts: 0,
        retry_delay_milliseconds: 500,
        max_connections: 10,
    };

    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("retry_attempts"));
}

#[test]
fn test_service_configuration_validate_zero_max_connections() {
    let config = ServiceConfiguration {
        enabled: true,
        timeout_seconds: 30,
        retry_attempts: 3,
        retry_delay_milliseconds: 500,
        max_connections: 0,
    };

    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("max_connections"));
}

#[test]
fn test_service_configuration_serialize() {
    let config = ServiceConfiguration::default();
    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("enabled"));
    assert!(json.contains("timeout_seconds"));
    assert!(json.contains("retry_attempts"));
}

#[test]
fn test_service_configuration_deserialize() {
    let json = r#"{
        "enabled": false,
        "timeout_seconds": 120,
        "retry_attempts": 5,
        "retry_delay_milliseconds": 2000,
        "max_connections": 20
    }"#;

    let config: ServiceConfiguration = serde_json::from_str(json).unwrap();
    assert!(!config.enabled);
    assert_eq!(config.timeout_seconds, 120);
    assert_eq!(config.retry_attempts, 5);
    assert_eq!(config.retry_delay_milliseconds, 2000);
    assert_eq!(config.max_connections, 20);
}

// ============================================================================
// RuntimeConfigurationBuilder Tests
// ============================================================================

#[test]
fn test_runtime_configuration_builder_defaults() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-1"), "Test Agent".to_string())
        .build();

    assert_eq!(config.agent_id.as_str(), "agent-1");
    assert_eq!(config.name, "Test Agent");
    assert_eq!(config.port, 8080);
    assert_eq!(config.host, "localhost");
    assert!(!config.ssl_enabled);
    assert!(!config.auth_required);
    assert!(config.system_prompt.is_none());
}

#[test]
fn test_runtime_configuration_builder_with_port() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-2"), "Agent".to_string())
        .port(9000)
        .build();

    assert_eq!(config.port, 9000);
}

#[test]
fn test_runtime_configuration_builder_with_host() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-3"), "Agent".to_string())
        .host("0.0.0.0".to_string())
        .build();

    assert_eq!(config.host, "0.0.0.0");
}

#[test]
fn test_runtime_configuration_builder_enable_ssl() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-4"), "Agent".to_string())
        .enable_ssl()
        .build();

    assert!(config.ssl_enabled);
}

#[test]
fn test_runtime_configuration_builder_require_auth() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-5"), "Agent".to_string())
        .require_auth()
        .build();

    assert!(config.auth_required);
}

#[test]
fn test_runtime_configuration_builder_with_system_prompt() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-6"), "Agent".to_string())
        .system_prompt("You are a helpful assistant".to_string())
        .build();

    assert_eq!(
        config.system_prompt,
        Some("You are a helpful assistant".to_string())
    );
}

#[test]
fn test_runtime_configuration_builder_chained() {
    let config = RuntimeConfigurationBuilder::new(AgentId::from("agent-7"), "Full Agent".to_string())
        .port(3000)
        .host("192.168.1.100".to_string())
        .enable_ssl()
        .require_auth()
        .system_prompt("Custom prompt".to_string())
        .build();

    assert_eq!(config.agent_id.as_str(), "agent-7");
    assert_eq!(config.name, "Full Agent");
    assert_eq!(config.port, 3000);
    assert_eq!(config.host, "192.168.1.100");
    assert!(config.ssl_enabled);
    assert!(config.auth_required);
    assert_eq!(config.system_prompt, Some("Custom prompt".to_string()));
}

// ============================================================================
// RuntimeConfiguration Tests
// ============================================================================

#[test]
fn test_runtime_configuration_serialize() {
    let config = RuntimeConfiguration {
        agent_id: AgentId::from("rt-1"),
        name: "Runtime Agent".to_string(),
        port: 8080,
        host: "localhost".to_string(),
        ssl_enabled: true,
        auth_required: true,
        system_prompt: Some("Prompt".to_string()),
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("rt-1"));
    assert!(json.contains("Runtime Agent"));
    assert!(json.contains("8080"));
}

#[test]
fn test_runtime_configuration_deserialize() {
    let json = r#"{
        "agent_id": "rt-2",
        "name": "Deserialized Agent",
        "port": 9999,
        "host": "example.com",
        "ssl_enabled": false,
        "auth_required": false,
        "system_prompt": null
    }"#;

    let config: RuntimeConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.agent_id.as_str(), "rt-2");
    assert_eq!(config.name, "Deserialized Agent");
    assert_eq!(config.port, 9999);
    assert_eq!(config.host, "example.com");
    assert!(!config.ssl_enabled);
    assert!(!config.auth_required);
    assert!(config.system_prompt.is_none());
}

// ============================================================================
// AgentServiceConfig Tests
// ============================================================================

#[test]
fn test_agent_service_config_default() {
    let config = AgentServiceConfig::default();

    assert!(!config.agent_id.as_str().is_empty());
    assert_eq!(config.name, "Default Agent");
    assert_eq!(config.description, "Default agent instance");
    assert_eq!(config.version, "0.1.0");
    assert_eq!(config.endpoint, "http://localhost:8080");
    assert_eq!(config.port, 8080);
    assert!(config.is_active);
}

#[test]
fn test_agent_service_config_validate_success() {
    let config = AgentServiceConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_agent_service_config_validate_empty_agent_id() {
    let config = AgentServiceConfig {
        agent_id: AgentId::from(""),
        name: "Test".to_string(),
        description: "Desc".to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost".to_string(),
        port: 8080,
        is_active: true,
    };

    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("agent_id"));
}

#[test]
fn test_agent_service_config_validate_zero_port() {
    let config = AgentServiceConfig {
        agent_id: AgentId::from("valid-id"),
        name: "Test".to_string(),
        description: "Desc".to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost".to_string(),
        port: 0,
        is_active: true,
    };

    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("port"));
}

#[test]
fn test_agent_service_config_validate_empty_name() {
    let config = AgentServiceConfig {
        agent_id: AgentId::from("valid-id"),
        name: "".to_string(),
        description: "Desc".to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost".to_string(),
        port: 8080,
        is_active: true,
    };

    let result = config.validate();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("name"));
}

#[test]
fn test_agent_service_config_serialize() {
    let config = AgentServiceConfig::default();
    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("agent_id"));
    assert!(json.contains("Default Agent"));
    assert!(json.contains("0.1.0"));
}

// ============================================================================
// ConnectionConfiguration Tests
// ============================================================================

#[test]
fn test_connection_configuration_timeout() {
    let config = ConnectionConfiguration {
        url: "http://example.com".to_string(),
        timeout_seconds: 45,
        keepalive_enabled: true,
        pool_size: 5,
    };

    assert_eq!(config.timeout(), Duration::from_secs(45));
}

#[test]
fn test_connection_configuration_serialize() {
    let config = ConnectionConfiguration {
        url: "https://api.example.com".to_string(),
        timeout_seconds: 60,
        keepalive_enabled: false,
        pool_size: 10,
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("https://api.example.com"));
    assert!(json.contains("60"));
    assert!(json.contains("10"));
}

#[test]
fn test_connection_configuration_deserialize() {
    let json = r#"{
        "url": "http://localhost:3000",
        "timeout_seconds": 30,
        "keepalive_enabled": true,
        "pool_size": 15
    }"#;

    let config: ConnectionConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(config.url, "http://localhost:3000");
    assert_eq!(config.timeout_seconds, 30);
    assert!(config.keepalive_enabled);
    assert_eq!(config.pool_size, 15);
}
