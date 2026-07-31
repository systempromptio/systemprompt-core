use systemprompt_mcp::services::client::{AuthChallenge, McpTransportError};
use systemprompt_models::mcp::McpExtensionId;
use systemprompt_models::oauth::ProtectedResourceMetadata;

#[test]
fn parses_a_no_credentials_challenge() {
    let challenge = AuthChallenge::parse(
        r#"Bearer realm="demo", resource_metadata="https://api.test/.well-known/oauth-protected-resource/api/v1/mcp/demo/mcp""#,
    );
    assert_eq!(challenge.realm.as_deref(), Some("demo"));
    assert_eq!(
        challenge.resource_metadata.as_deref(),
        Some("https://api.test/.well-known/oauth-protected-resource/api/v1/mcp/demo/mcp")
    );
    assert!(challenge.error.is_none());
}

#[test]
fn parses_an_invalid_token_challenge_with_a_comma_inside_the_description() {
    let challenge = AuthChallenge::parse(
        r#"Bearer realm="demo", error="invalid_token", error_description="expired, re-authenticate""#,
    );
    assert_eq!(challenge.error.as_deref(), Some("invalid_token"));
    assert_eq!(
        challenge.error_description.as_deref(),
        Some("expired, re-authenticate"),
        "a quoted comma must not split the parameter list"
    );
}

#[test]
fn ignores_a_non_bearer_scheme() {
    let challenge = AuthChallenge::parse(r#"Basic realm="demo""#);
    assert_eq!(challenge, AuthChallenge::default());
}

#[test]
fn detects_the_enterprise_managed_auth_extension() {
    let metadata: ProtectedResourceMetadata = serde_json::from_value(serde_json::json!({
        "resource": "https://api.test/api/v1/mcp/demo/mcp",
        "authorization_servers": ["https://api.test"],
        "scopes_supported": ["user"],
        "mcp_extensions_supported": [
            "io.modelcontextprotocol/enterprise-managed-authorization"
        ],
    }))
    .expect("metadata deserializes");
    assert!(metadata.requires_enterprise_managed_auth());
    assert_eq!(
        metadata.mcp_extensions_supported,
        vec![McpExtensionId::EnterpriseManagedAuth]
    );
}

#[test]
fn plain_oauth_metadata_does_not_require_enterprise_managed_auth() {
    let metadata: ProtectedResourceMetadata = serde_json::from_value(serde_json::json!({
        "resource": "https://api.test/api/v1/mcp/demo/mcp",
        "authorization_servers": ["https://api.test"],
    }))
    .expect("metadata deserializes");
    assert!(!metadata.requires_enterprise_managed_auth());
    assert!(metadata.scopes_supported.is_empty());
}

#[test]
fn an_enterprise_managed_challenge_is_typed_not_stringly() {
    let error = McpTransportError::AuthorizationRequired {
        reason: "the server requires authorization".to_owned(),
        resource: Some("https://api.test/api/v1/mcp/demo/mcp".to_owned()),
        metadata_url: Some("https://api.test/.well-known/oauth-protected-resource".to_owned()),
        authorization_servers: vec!["https://idp.test".to_owned()],
        enterprise_managed: true,
    };

    let McpTransportError::AuthorizationRequired {
        enterprise_managed,
        resource,
        authorization_servers,
        ..
    } = &error
    else {
        panic!("expected an authorization-required error");
    };
    assert!(
        *enterprise_managed,
        "a caller must be able to branch on EMA without parsing a message"
    );
    assert_eq!(
        resource.as_deref(),
        Some("https://api.test/api/v1/mcp/demo/mcp")
    );
    assert_eq!(authorization_servers, &["https://idp.test".to_owned()]);
    assert!(
        error.to_string().contains("enterprise-managed"),
        "the rendered message still names the flow: {error}"
    );
}
