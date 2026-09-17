use std::collections::HashMap;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::{Deployment, ExternalAuth, McpServerType, OAuthRequirement};

fn deployment(server_type: McpServerType, endpoint: Option<&str>) -> Deployment {
    let spawned = server_type == McpServerType::Internal;
    Deployment {
        connector: None,
        server_type,
        binary: spawned.then(|| "bin".to_owned()),
        package: None,
        port: spawned.then_some(5100),
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
            ema: false,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::new(),
        tool_policy: None,
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

#[test]
fn external_server_without_spawn_fields_parses_and_validates() {
    let d: Deployment = serde_yaml::from_str(
        "type: external\nendpoint: https://mcp.example.com/mcp\nenabled: true\n\
         display_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  \
         client_id: null\n",
    )
    .expect("external server needs no binary, package or port");
    assert_eq!(d.binary, None);
    assert_eq!(d.package, None);
    assert_eq!(d.port, None);
    d.validate("remote").expect("external server validates");
}

#[test]
fn external_server_declaring_port_is_rejected() {
    let mut d = deployment(McpServerType::External, Some("https://mcp.example.com/mcp"));
    d.port = Some(5046);
    let msg = d.validate("remote").unwrap_err().to_string();
    assert!(msg.contains("remote") && msg.contains("port"), "{msg}");
}

#[test]
fn external_server_declaring_binary_is_rejected() {
    let mut d = deployment(McpServerType::External, Some("https://mcp.example.com/mcp"));
    d.binary = Some(String::new());
    let msg = d.validate("remote").unwrap_err().to_string();
    assert!(msg.contains("binary"), "{msg}");
}

#[test]
fn external_server_without_endpoint_is_rejected() {
    let d = deployment(McpServerType::External, None);
    let msg = d.validate("remote").unwrap_err().to_string();
    assert!(msg.contains("endpoint"), "{msg}");
}

#[test]
fn internal_server_without_port_is_rejected() {
    let mut d = deployment(McpServerType::Internal, None);
    d.port = None;
    let msg = d.validate("local").unwrap_err().to_string();
    assert!(msg.contains("local") && msg.contains("port"), "{msg}");
}

#[test]
fn internal_server_without_binary_is_rejected() {
    let mut d = deployment(McpServerType::Internal, None);
    d.binary = Some("  ".to_owned());
    let msg = d.validate("local").unwrap_err().to_string();
    assert!(msg.contains("binary"), "{msg}");
}
