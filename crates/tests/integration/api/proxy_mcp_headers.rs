//! Outbound header construction for proxied MCP and agent requests, and the
//! external-MCP resolve-error mapping.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_api::services::proxy::ProxyError;
use systemprompt_api::services::proxy::service::external::{map_resolve_error, outbound_headers};
use systemprompt_api::services::proxy::service::outbound::{agent_headers, managed_mcp_headers};
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::McpDomainError;

use super::common::request_context;

fn forged() -> HeaderMap {
    let mut incoming = HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    incoming.insert("mcp-session-id", HeaderValue::from_static("sess"));
    incoming.insert("x-proxy-verified", HeaderValue::from_static("true"));
    incoming.insert("x-user-permissions", HeaderValue::from_static("admin"));
    incoming.insert("x-user-roles", HeaderValue::from_static("admin"));
    incoming.insert("x-user-id", HeaderValue::from_static("forged-user"));
    incoming.insert("x-context-id", HeaderValue::from_static("forged-context"));
    incoming.insert("x-systemprompt-owner-hop", HeaderValue::from_static("1"));
    incoming.insert("x-systemprompt-scope-admin", HeaderValue::from_static("1"));
    incoming.insert("authorization", HeaderValue::from_static("Bearer forged"));
    incoming.insert("cookie", HeaderValue::from_static("secret=1"));
    incoming
}

fn assert_no_forged_identity(out: &HeaderMap, context_user: &str) {
    assert!(out.get("x-proxy-verified").is_none());
    assert!(out.get("x-user-permissions").is_none());
    assert!(out.get("x-user-roles").is_none());
    assert!(out.get("x-systemprompt-owner-hop").is_none());
    assert!(out.get("x-systemprompt-scope-admin").is_none());
    assert!(out.get("authorization").is_none());
    assert_eq!(
        out.get("x-user-id").and_then(|v| v.to_str().ok()),
        Some(context_user)
    );
    assert_ne!(
        out.get("x-context-id").and_then(|v| v.to_str().ok()),
        Some("forged-context")
    );
}

#[test]
fn managed_mcp_headers_carry_only_protocol_headers_and_proxy_identity() {
    let rc = request_context("headers-mcp");
    let out = managed_mcp_headers(&forged(), &rc);
    assert_no_forged_identity(&out, rc.user_id().as_str());
    assert!(out.get("cookie").is_none());
    assert_eq!(
        out.get("mcp-session-id").and_then(|v| v.to_str().ok()),
        Some("sess")
    );
    assert_eq!(
        out.get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
}

#[test]
fn agent_headers_drop_every_client_supplied_identity_header() {
    let rc = request_context("headers-agent");
    let out = agent_headers(&forged(), &rc);
    assert_no_forged_identity(&out, rc.user_id().as_str());
    assert_eq!(
        out.get("cookie").and_then(|v| v.to_str().ok()),
        Some("secret=1")
    );
}

#[test]
fn outbound_headers_filters_to_mcp_passthrough_set() {
    let mut incoming = HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    incoming.insert("accept", HeaderValue::from_static("application/json"));
    incoming.insert("mcp-session-id", HeaderValue::from_static("sess"));
    incoming.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2025-01-01"),
    );
    incoming.insert("authorization", HeaderValue::from_static("Bearer leak-me"));
    incoming.insert("cookie", HeaderValue::from_static("secret=1"));

    let out = outbound_headers(&incoming, std::collections::HashMap::new());
    assert!(
        out.get("authorization").is_none(),
        "client bearer must not leak"
    );
    assert!(out.get("cookie").is_none());
    assert_eq!(out.len(), 4);
}

#[test]
fn outbound_headers_provider_credential_wins() {
    let mut incoming = HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    let provider = std::collections::HashMap::from([(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer provider-token"),
    )]);
    let out = outbound_headers(&incoming, provider);
    assert_eq!(
        out.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer provider-token")
    );
}

#[test]
fn resolve_error_mapping_covers_auth_and_availability() {
    let auth = map_resolve_error(
        &ServiceName::new("ext"),
        McpDomainError::AuthRequired("login".to_owned()),
    )
    .to_string();
    assert!(auth.contains("Authentication required"), "{auth}");

    let unavailable = map_resolve_error(
        &ServiceName::new("ext"),
        McpDomainError::ExternalAuthUnavailable {
            server: "ext".to_owned(),
            message: "vault down".to_owned(),
        },
    )
    .to_string();
    assert!(unavailable.contains("vault down"), "{unavailable}");

    let other = map_resolve_error(
        &ServiceName::new("ext"),
        McpDomainError::transport("token accessor", std::io::Error::other("boom")),
    );
    assert!(
        matches!(other, ProxyError::ExternalResolveFailed { ref service, .. } if service == "ext"),
        "{other:?}"
    );
    assert!(!other.to_string().contains("boom"), "{other}");
}
