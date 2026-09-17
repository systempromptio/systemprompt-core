//! Tests for registry.rs error types.

use systemprompt_traits::registry::{AgentInfo, McpServerInfo, RegistryError, ServiceOAuthConfig};

// --- RegistryError display ---

#[test]
fn registry_not_found_display() {
    let e = RegistryError::NotFound("my-agent".to_owned());
    assert!(format!("{e}").contains("my-agent"));
}

#[test]
fn registry_unavailable_display() {
    let e = RegistryError::Unavailable("no connection".to_owned());
    assert!(format!("{e}").contains("no connection"));
}

#[test]
fn registry_configuration_display() {
    let e = RegistryError::Configuration("bad port".to_owned());
    assert!(format!("{e}").contains("bad port"));
}

#[test]
fn registry_internal_display() {
    let e = RegistryError::Internal("panic".to_owned());
    assert!(format!("{e}").contains("panic"));
}

#[test]
fn registry_errors_are_debug() {
    let variants: &[RegistryError] = &[
        RegistryError::NotFound("a".into()),
        RegistryError::Unavailable("b".into()),
        RegistryError::Configuration("c".into()),
        RegistryError::Internal("d".into()),
    ];
    for e in variants {
        assert!(!format!("{e:?}").is_empty());
    }
}

// --- ServiceOAuthConfig ---

#[test]
fn service_oauth_config_default_required_true() {
    let c = ServiceOAuthConfig::default();
    assert!(c.required);
    assert!(c.scopes.is_empty());
    assert!(c.audience.is_empty());
    assert!(!c.ema, "EMA is opt-in per service");
}


// --- AgentInfo ---

#[test]
fn agent_info_fields_accessible() {
    let a = AgentInfo {
        name: "my-agent".to_owned(),
        port: 9000,
        enabled: true,
        oauth: ServiceOAuthConfig::default(),
    };
    assert_eq!(a.name, "my-agent");
    assert_eq!(a.port, 9000);
    assert!(a.enabled);
}


// --- McpServerInfo ---

#[test]
fn mcp_server_info_fields_accessible() {
    let s = McpServerInfo {
        name: "mcp-server".to_owned(),
        port: Some(3000),
        enabled: true,
        oauth: ServiceOAuthConfig::default(),
    };
    assert_eq!(s.name, "mcp-server");
    assert_eq!(s.port, Some(3000));
}
