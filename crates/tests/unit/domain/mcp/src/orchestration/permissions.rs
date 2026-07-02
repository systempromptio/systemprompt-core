//! Tests for the per-server OAuth permission filter used by the tool loader.

use std::collections::HashMap;
use systemprompt_mcp::orchestration::has_server_permission;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::deployment::{Deployment, McpServerType, OAuthRequirement};
use systemprompt_models::services::ServicesConfig;

fn deployment(required: bool, scopes: Vec<Permission>) -> Deployment {
    Deployment {
        server_type: McpServerType::Internal,
        binary: "bin".to_owned(),
        package: None,
        port: 5001,
        endpoint: None,
        enabled: true,
        display_in_web: true,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required,
            scopes,
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::default(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::default(),
    }
}

fn config_with(name: &str, dep: Deployment) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(name.to_owned(), dep);
    config
}

#[test]
fn unknown_server_is_allowed() {
    let config = ServicesConfig::default();
    assert!(has_server_permission(&config, "ghost", &[]));
}

#[test]
fn oauth_not_required_is_allowed_without_permissions() {
    let config = config_with("open", deployment(false, vec![Permission::Admin]));
    assert!(has_server_permission(&config, "open", &[]));
}

#[test]
fn oauth_required_with_empty_scopes_is_allowed() {
    let config = config_with("scopeless", deployment(true, vec![]));
    assert!(has_server_permission(&config, "scopeless", &[]));
}

#[test]
fn required_scope_without_matching_permission_is_denied() {
    let config = config_with("locked", deployment(true, vec![Permission::Admin]));
    assert!(!has_server_permission(
        &config,
        "locked",
        &[Permission::Anonymous]
    ));
    assert!(!has_server_permission(&config, "locked", &[]));
}

#[test]
fn permission_implying_required_scope_is_allowed() {
    let config = config_with("locked", deployment(true, vec![Permission::User]));
    assert!(has_server_permission(
        &config,
        "locked",
        &[Permission::Admin]
    ));
    assert!(has_server_permission(
        &config,
        "locked",
        &[Permission::User]
    ));
}
