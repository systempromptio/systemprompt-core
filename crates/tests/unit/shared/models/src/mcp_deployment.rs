use std::collections::HashMap;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::{Deployment, McpServerType, OAuthRequirement};

fn deployment(server_type: McpServerType, endpoint: Option<&str>) -> Deployment {
    Deployment {
        server_type,
        binary: "bin".to_owned(),
        package: None,
        port: 5100,
        endpoint: endpoint.map(str::to_owned),
        enabled: true,
        display_in_web: false,
        dev_only: false,
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
    }
}

#[test]
fn internal_endpoint_absolute_url_is_rejected() {
    let d = deployment(
        McpServerType::Internal,
        Some("http://localhost:8080/api/v1/mcp/fixture/mcp"),
    );
    let err = d.validate("fixture").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("fixture"),
        "error should mention server name: {msg}"
    );
    assert!(
        msg.contains("relative path"),
        "error should mention relative path requirement: {msg}"
    );
}

#[test]
fn internal_endpoint_relative_is_accepted() {
    let d = deployment(McpServerType::Internal, Some("/api/v1/mcp/fixture/mcp"));
    d.validate("fixture")
        .expect("relative endpoint must be accepted");
}

#[test]
fn internal_endpoint_none_is_accepted() {
    let d = deployment(McpServerType::Internal, None);
    d.validate("fixture")
        .expect("absent endpoint must be accepted");
}

#[test]
fn external_endpoint_absolute_url_is_accepted() {
    let d = deployment(
        McpServerType::External,
        Some("https://example.com/upstream/mcp"),
    );
    d.validate("upstream")
        .expect("external servers may use absolute URLs");
}
