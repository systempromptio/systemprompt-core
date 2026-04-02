use systemprompt_mcp::services::registry::validator::validate_registry;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::registry::RegistryConfig;
use systemprompt_models::mcp::server::McpServerConfig;
use std::path::PathBuf;

fn make_internal_server(name: &str, port: u16) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: format!("{name} MCP Server"),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_string(),
        host: "0.0.0.0".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: String::new(),
    }
}

fn make_external_server(name: &str, endpoint: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        server_type: McpServerType::External,
        binary: String::new(),
        enabled: true,
        display_in_web: true,
        port: 0,
        crate_path: PathBuf::new(),
        display_name: format!("{name} Server"),
        description: format!("{name} MCP Server"),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_string(),
        host: "0.0.0.0".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: endpoint.to_string(),
    }
}

fn make_config(servers: Vec<McpServerConfig>) -> RegistryConfig {
    RegistryConfig {
        servers,
        registry_url: None,
        cache_dir: None,
    }
}

#[test]
fn validate_registry_empty_servers() {
    let config = make_config(vec![]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_single_valid_internal() {
    let config = make_config(vec![make_internal_server("test-server", 5000)]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_multiple_valid_no_port_conflict() {
    let config = make_config(vec![
        make_internal_server("server-a", 5000),
        make_internal_server("server-b", 5001),
    ]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_port_conflict_detected() {
    let config = make_config(vec![
        make_internal_server("server-a", 5000),
        make_internal_server("server-b", 5000),
    ]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Port conflict"));
}

#[test]
fn validate_registry_disabled_servers_no_port_conflict() {
    let mut server_a = make_internal_server("server-a", 5000);
    let mut server_b = make_internal_server("server-b", 5000);
    server_a.enabled = false;
    server_b.enabled = false;
    let config = make_config(vec![server_a, server_b]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_low_port_rejected() {
    let config = make_config(vec![make_internal_server("server", 80)]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("invalid port"));
}

#[test]
fn validate_registry_port_1023_rejected() {
    let config = make_config(vec![make_internal_server("server", 1023)]);
    let result = validate_registry(&config);
    assert!(result.is_err());
}

#[test]
fn validate_registry_port_1024_accepted() {
    let config = make_config(vec![make_internal_server("server", 1024)]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_missing_display_name() {
    let mut server = make_internal_server("server", 5000);
    server.display_name = String::new();
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("display_name"));
}

#[test]
fn validate_registry_missing_description() {
    let mut server = make_internal_server("server", 5000);
    server.description = String::new();
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("description"));
}

#[test]
fn validate_registry_oauth_required_but_no_scopes() {
    let mut server = make_internal_server("server", 5000);
    server.oauth.required = true;
    server.oauth.scopes = vec![];
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("OAuth"));
}

#[test]
fn validate_registry_oauth_required_with_scopes_ok() {
    let mut server = make_internal_server("server", 5000);
    server.oauth.required = true;
    server.oauth.scopes = vec![Permission::User];
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_oauth_not_required_empty_scopes_ok() {
    let server = make_internal_server("server", 5000);
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_internal_server_no_binary() {
    let mut server = make_internal_server("server", 5000);
    server.binary = String::new();
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("binary"));
}

#[test]
fn validate_registry_external_server_valid() {
    let config = make_config(vec![make_external_server("ext", "https://example.com/mcp")]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_external_server_no_endpoint() {
    let mut server = make_external_server("ext", "");
    server.remote_endpoint = String::new();
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("remote endpoint"));
}

#[test]
fn validate_registry_external_server_with_binary_rejected() {
    let mut server = make_external_server("ext", "https://example.com/mcp");
    server.binary = "some-binary".to_string();
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("should not have a binary"));
}

#[test]
fn validate_registry_disabled_server_skips_validation() {
    let mut server = make_internal_server("server", 80);
    server.enabled = false;
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_mixed_internal_external() {
    let config = make_config(vec![
        make_internal_server("internal-svc", 5000),
        make_external_server("external-svc", "https://example.com/mcp"),
    ]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_external_servers_no_port_conflict() {
    let ext1 = make_external_server("ext1", "https://a.com/mcp");
    let ext2 = make_external_server("ext2", "https://b.com/mcp");
    let config = make_config(vec![ext1, ext2]);
    let result = validate_registry(&config);
    assert!(result.is_ok());
}

#[test]
fn validate_registry_internal_nonexistent_crate_path() {
    let mut server = make_internal_server("server", 5000);
    server.crate_path = PathBuf::from("/nonexistent/path/to/crate");
    let config = make_config(vec![server]);
    let result = validate_registry(&config);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("does not exist"));
}
