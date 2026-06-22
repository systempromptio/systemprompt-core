use std::collections::HashMap;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::{Deployment, ExternalAuth, McpServerType, OAuthRequirement};

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
        external_auth: None,
        headers: HashMap::new(),
    }
}

fn external_auth(token_endpoint: &str) -> ExternalAuth {
    ExternalAuth {
        token_endpoint: token_endpoint.to_owned(),
        header: "Authorization".to_owned(),
        scheme: "Bearer".to_owned(),
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

#[test]
fn external_auth_relative_token_endpoint_is_accepted() {
    let mut d = deployment(McpServerType::External, Some("https://example.com/mcp"));
    d.external_auth = Some(external_auth("/api/public/salesforce/token"));
    d.validate("salesforce")
        .expect("relative token_endpoint must be accepted");
}

#[test]
fn external_auth_absolute_token_endpoint_is_rejected() {
    let mut d = deployment(McpServerType::External, Some("https://example.com/mcp"));
    d.external_auth = Some(external_auth("https://idp.example.com/token"));
    let msg = d.validate("salesforce").unwrap_err().to_string();
    assert!(
        msg.contains("relative") && msg.contains("salesforce"),
        "absolute token_endpoint must be rejected: {msg}"
    );
}

#[test]
fn external_auth_non_rooted_token_endpoint_is_rejected() {
    let mut d = deployment(McpServerType::External, Some("https://example.com/mcp"));
    d.external_auth = Some(external_auth("api/public/salesforce/token"));
    let msg = d.validate("salesforce").unwrap_err().to_string();
    assert!(
        msg.contains("'/'"),
        "token_endpoint without leading slash must be rejected: {msg}"
    );
}

#[test]
fn external_auth_empty_header_is_rejected() {
    let mut d = deployment(McpServerType::External, Some("https://example.com/mcp"));
    let mut ext = external_auth("/api/public/salesforce/token");
    ext.header = String::new();
    d.external_auth = Some(ext);
    let msg = d.validate("salesforce").unwrap_err().to_string();
    assert!(
        msg.contains("header"),
        "empty header must be rejected: {msg}"
    );
}

#[test]
fn external_auth_on_internal_server_is_rejected() {
    let mut d = deployment(McpServerType::Internal, None);
    d.external_auth = Some(external_auth("/api/public/salesforce/token"));
    let msg = d.validate("fixture").unwrap_err().to_string();
    assert!(
        msg.contains("external servers"),
        "external_auth on an internal server must be rejected: {msg}"
    );
}

#[test]
fn external_auth_header_value_prefixes_scheme() {
    let ext = external_auth("/api/public/salesforce/token");
    assert_eq!(ext.header_value("sf-xyz"), "Bearer sf-xyz");
}

#[test]
fn external_auth_header_value_empty_scheme_is_raw_token() {
    let mut ext = external_auth("/api/public/acme/token");
    ext.scheme = String::new();
    assert_eq!(ext.header_value("raw-key"), "raw-key");
}

#[test]
fn static_headers_on_internal_server_are_rejected() {
    let mut d = deployment(McpServerType::Internal, None);
    d.headers
        .insert("X-Api-Key".to_owned(), "secret".to_owned());
    let msg = d.validate("fixture").unwrap_err().to_string();
    assert!(
        msg.contains("external servers"),
        "static headers on an internal server must be rejected: {msg}"
    );
}
