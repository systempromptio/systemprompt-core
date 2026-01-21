//! Unit tests for API models
//!
//! Tests cover:
//! - ServerConfig default values
//! - ServerConfig construction and field access
//! - ServerConfig Clone and Debug traits

use systemprompt_api::ServerConfig;

// ============================================================================
// ServerConfig Default Tests
// ============================================================================

#[test]
fn test_server_config_default_host() {
    let config = ServerConfig::default();
    assert_eq!(config.host, "0.0.0.0");
}

#[test]
fn test_server_config_default_port() {
    let config = ServerConfig::default();
    assert_eq!(config.port, 8080);
}

#[test]
fn test_server_config_default_values() {
    let config = ServerConfig::default();
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8080);
}

// ============================================================================
// ServerConfig Construction Tests
// ============================================================================

#[test]
fn test_server_config_custom_host() {
    let config = ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
    };
    assert_eq!(config.host, "127.0.0.1");
}

#[test]
fn test_server_config_custom_port() {
    let config = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 3000,
    };
    assert_eq!(config.port, 3000);
}

#[test]
fn test_server_config_custom_values() {
    let config = ServerConfig {
        host: "localhost".to_string(),
        port: 9000,
    };
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 9000);
}

#[test]
fn test_server_config_ipv6_host() {
    let config = ServerConfig {
        host: "::1".to_string(),
        port: 8080,
    };
    assert_eq!(config.host, "::1");
}

#[test]
fn test_server_config_empty_host() {
    let config = ServerConfig {
        host: String::new(),
        port: 8080,
    };
    assert!(config.host.is_empty());
}

// ============================================================================
// ServerConfig Boundary Tests
// ============================================================================

#[test]
fn test_server_config_port_zero() {
    let config = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 0,
    };
    assert_eq!(config.port, 0);
}

#[test]
fn test_server_config_port_max() {
    let config = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 65535,
    };
    assert_eq!(config.port, 65535);
}

#[test]
fn test_server_config_common_ports() {
    let ports = vec![80, 443, 3000, 8000, 8080, 8443];
    for port in ports {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            port,
        };
        assert_eq!(config.port, port);
    }
}

// ============================================================================
// ServerConfig Clone Tests
// ============================================================================

#[test]
fn test_server_config_clone() {
    let original = ServerConfig {
        host: "192.168.1.1".to_string(),
        port: 5000,
    };
    let cloned = original.clone();
    assert_eq!(cloned.host, "192.168.1.1");
    assert_eq!(cloned.port, 5000);
}

#[test]
fn test_server_config_clone_independence() {
    let original = ServerConfig {
        host: "original".to_string(),
        port: 1000,
    };
    let mut cloned = original.clone();
    cloned.host = "modified".to_string();
    cloned.port = 2000;

    assert_eq!(original.host, "original");
    assert_eq!(original.port, 1000);
    assert_eq!(cloned.host, "modified");
    assert_eq!(cloned.port, 2000);
}

// ============================================================================
// ServerConfig Debug Tests
// ============================================================================

#[test]
fn test_server_config_debug() {
    let config = ServerConfig {
        host: "test-host".to_string(),
        port: 4000,
    };
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("ServerConfig"));
    assert!(debug_str.contains("test-host"));
    assert!(debug_str.contains("4000"));
}

#[test]
fn test_server_config_debug_default() {
    let config = ServerConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("0.0.0.0"));
    assert!(debug_str.contains("8080"));
}
