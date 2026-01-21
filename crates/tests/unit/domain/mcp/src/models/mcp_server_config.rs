//! Unit tests for McpServerConfig

use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_mcp::McpServerConfig;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::OAuthRequirement;

fn create_test_config() -> McpServerConfig {
    McpServerConfig {
        name: "test-service".to_string(),
        binary: "test-binary".to_string(),
        enabled: true,
        display_in_web: true,
        port: 8080,
        crate_path: PathBuf::from("/path/to/crate"),
        display_name: "Test Service".to_string(),
        description: "A test MCP service".to_string(),
        capabilities: vec!["tools".to_string(), "prompts".to_string()],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "1.0.0".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
    }
}

// ============================================================================
// McpServerConfig endpoint Tests
// ============================================================================

#[test]
fn test_mcp_server_config_endpoint() {
    let config = create_test_config();
    let endpoint = config.endpoint("http://localhost:3000");
    assert_eq!(endpoint, "http://localhost:3000/api/v1/mcp/test-service/mcp");
}

#[test]
fn test_mcp_server_config_endpoint_with_trailing_slash() {
    let config = create_test_config();
    let endpoint = config.endpoint("http://localhost:3000/");
    // Note: this will produce a double slash - just documenting behavior
    assert!(endpoint.contains("test-service"));
}

#[test]
fn test_mcp_server_config_endpoint_https() {
    let config = create_test_config();
    let endpoint = config.endpoint("https://api.example.com");
    assert!(endpoint.starts_with("https://"));
    assert!(endpoint.contains("test-service"));
}

#[test]
fn test_mcp_server_config_endpoint_different_names() {
    let mut config = create_test_config();
    config.name = "my-custom-service".to_string();

    let endpoint = config.endpoint("http://localhost");
    assert!(endpoint.contains("my-custom-service"));
}

// ============================================================================
// McpServerConfig Field Access Tests
// ============================================================================

#[test]
fn test_mcp_server_config_fields() {
    let config = create_test_config();

    assert_eq!(config.name, "test-service");
    assert_eq!(config.binary, "test-binary");
    assert!(config.enabled);
    assert!(config.display_in_web);
    assert_eq!(config.port, 8080);
    assert_eq!(config.display_name, "Test Service");
    assert_eq!(config.description, "A test MCP service");
    assert_eq!(config.version, "1.0.0");
    assert_eq!(config.host, "127.0.0.1");
}

#[test]
fn test_mcp_server_config_disabled() {
    let mut config = create_test_config();
    config.enabled = false;
    assert!(!config.enabled);
}

#[test]
fn test_mcp_server_config_oauth_required() {
    let mut config = create_test_config();
    config.oauth.required = true;
    config.oauth.scopes = vec![Permission::Admin];

    assert!(config.oauth.required);
    assert!(!config.oauth.scopes.is_empty());
}

#[test]
fn test_mcp_server_config_with_capabilities() {
    let config = create_test_config();
    assert!(config.capabilities.contains(&"tools".to_string()));
    assert!(config.capabilities.contains(&"prompts".to_string()));
}

#[test]
fn test_mcp_server_config_with_env_vars() {
    let mut config = create_test_config();
    config.env_vars = vec!["DATABASE_URL".to_string(), "API_KEY".to_string()];

    assert_eq!(config.env_vars.len(), 2);
    assert!(config.env_vars.contains(&"DATABASE_URL".to_string()));
}

// ============================================================================
// McpServerConfig Clone Tests
// ============================================================================

#[test]
fn test_mcp_server_config_clone() {
    let config = create_test_config();
    let cloned = config.clone();

    assert_eq!(config.name, cloned.name);
    assert_eq!(config.port, cloned.port);
    assert_eq!(config.enabled, cloned.enabled);
    assert_eq!(config.host, cloned.host);
}

// ============================================================================
// McpServerConfig Debug Tests
// ============================================================================

#[test]
fn test_mcp_server_config_debug() {
    let config = create_test_config();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("McpServerConfig"));
    assert!(debug_str.contains("test-service"));
}

// ============================================================================
// McpServerConfig Serialization Tests
// ============================================================================

#[test]
fn test_mcp_server_config_serialize() {
    let config = create_test_config();
    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("test-service"));
    assert!(json.contains("8080"));
    assert!(json.contains("1.0.0"));
}

#[test]
fn test_mcp_server_config_deserialize() {
    let config = create_test_config();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: McpServerConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.port, deserialized.port);
    assert_eq!(config.enabled, deserialized.enabled);
}

#[test]
fn test_mcp_server_config_roundtrip() {
    let config = create_test_config();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: McpServerConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.version, deserialized.version);
    assert_eq!(config.host, deserialized.host);
}
