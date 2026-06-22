use std::path::PathBuf;
use systemprompt_mcp::services::registry::validator::validate_registry;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::registry::RegistryConfig;
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn make_internal(name: &str, port: u16) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Display"),
        description: format!("{name} Description"),
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
        version: "1.0.0".to_owned(),
        host: "0.0.0.0".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn make_external(name: &str, endpoint: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::External,
        binary: String::new(),
        enabled: true,
        display_in_web: true,
        port: 0,
        crate_path: PathBuf::new(),
        display_name: format!("{name} Display"),
        description: format!("{name} Description"),
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
        version: "1.0.0".to_owned(),
        host: "0.0.0.0".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: endpoint.to_owned(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn registry(servers: Vec<McpServerConfig>) -> RegistryConfig {
    RegistryConfig {
        servers,
        registry_url: None,
        cache_dir: None,
    }
}

#[test]
fn three_servers_all_different_ports() {
    let cfg = registry(vec![
        make_internal("a", 5000),
        make_internal("b", 5001),
        make_internal("c", 5002),
    ]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn three_servers_one_port_conflict() {
    let cfg = registry(vec![
        make_internal("a", 5000),
        make_internal("b", 5001),
        make_internal("c", 5001),
    ]);
    let err = validate_registry(&cfg).unwrap_err();
    assert!(err.to_string().contains("conflict") || err.to_string().contains("Port"));
}

#[test]
fn disabled_server_not_checked_for_binary() {
    let mut srv = make_internal("srv", 5000);
    srv.enabled = false;
    srv.binary = String::new();
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn disabled_server_missing_display_name_ok() {
    let mut srv = make_internal("srv", 5000);
    srv.enabled = false;
    srv.display_name = String::new();
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn external_server_with_oauth_scopes_ok() {
    let mut srv = make_external("ext", "https://api.example.com/mcp");
    srv.oauth.required = true;
    srv.oauth.scopes = vec![Permission::User];
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn external_server_oauth_required_no_scopes_fails() {
    let mut srv = make_external("ext", "https://api.example.com/mcp");
    srv.oauth.required = true;
    srv.oauth.scopes = vec![];
    let cfg = registry(vec![srv]);
    let err = validate_registry(&cfg).unwrap_err();
    assert!(err.to_string().contains("OAuth") || err.to_string().contains("scope"));
}

#[test]
fn port_1024_boundary_ok() {
    let srv = make_internal("srv", 1024);
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn port_1023_rejected() {
    let srv = make_internal("srv", 1023);
    let cfg = registry(vec![srv]);
    let err = validate_registry(&cfg).unwrap_err();
    assert!(err.to_string().contains("port") || err.to_string().contains("invalid"));
}

#[test]
fn port_0_rejected() {
    let srv = make_internal("srv", 0);
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_err());
}

#[test]
fn mixed_enabled_disabled_port_conflict_only_enabled_count() {
    let mut disabled = make_internal("disabled", 5000);
    disabled.enabled = false;
    let enabled = make_internal("enabled", 5000);
    let cfg = registry(vec![disabled, enabled]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn two_enabled_same_port_conflict() {
    let a = make_internal("a", 6000);
    let b = make_internal("b", 6000);
    let cfg = registry(vec![a, b]);
    assert!(validate_registry(&cfg).is_err());
}

#[test]
fn many_external_servers_no_port_conflict() {
    let cfg = registry(vec![
        make_external("ext-1", "https://a.com/mcp"),
        make_external("ext-2", "https://b.com/mcp"),
        make_external("ext-3", "https://c.com/mcp"),
    ]);
    assert!(validate_registry(&cfg).is_ok());
}

#[test]
fn internal_missing_both_display_name_and_description() {
    let mut srv = make_internal("srv", 5000);
    srv.display_name = String::new();
    srv.description = String::new();
    let cfg = registry(vec![srv]);
    let err = validate_registry(&cfg).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("display_name") || msg.contains("description"));
}

#[test]
fn external_server_empty_endpoint() {
    let srv = make_external("ext", "");
    let cfg = registry(vec![srv]);
    assert!(validate_registry(&cfg).is_err());
}

#[test]
fn valid_config_returns_unit() {
    let cfg = registry(vec![make_internal("ok-srv", 8765)]);
    let result = validate_registry(&cfg);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ());
}
