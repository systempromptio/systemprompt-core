//! Tests for the proxy-verified identity short-circuit: header extraction,
//! scope enforcement, and the authenticated context it produces.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::OAuthRequirement;
use systemprompt_mcp::middleware::rbac::try_proxy_verified_auth;
use systemprompt_mcp::middleware::{AuthResult, AuthenticatedRequestContext};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{JwtAudience, Permission};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-proxy"),
        TraceId::new("t-proxy"),
        ContextId::generate(),
        AgentName::new("agent-proxy"),
    )
}

fn oauth(scopes: Vec<Permission>) -> OAuthRequirement {
    OAuthRequirement {
        required: true,
        scopes,
        audience: JwtAudience::Mcp,
        client_id: None,
    }
}

fn parts_with(headers: &[(&str, &str)]) -> http::request::Parts {
    let mut builder = http::Request::builder().method("POST").uri("/mcp");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let (parts, ()) = builder.body(()).expect("request builds").into_parts();
    parts
}

const USER_ID: &str = "6dfe8bd7-1a86-4a52-a353-6a9ff9a3f0f0";

fn verified_headers<'a>() -> Vec<(&'a str, &'a str)> {
    vec![
        ("x-proxy-verified", "true"),
        ("x-user-id", USER_ID),
        ("x-user-permissions", "user mcp"),
        ("authorization", "Bearer proxied-token"),
    ]
}

#[test]
fn missing_parts_is_an_error() {
    let err = try_proxy_verified_auth(None, ctx(), &oauth(vec![]), "srv")
        .expect_err("missing parts rejected");
    assert!(err.to_string().contains("No HTTP parts"));
}

#[test]
fn unverified_request_passes_through() {
    let parts = parts_with(&[("authorization", "Bearer tok")]);
    let result = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![]), "srv")
        .expect("passthrough ok");
    assert!(result.is_none());
}

#[test]
fn verified_header_false_passes_through() {
    let parts = parts_with(&[("x-proxy-verified", "false")]);
    let result = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![]), "srv")
        .expect("passthrough ok");
    assert!(result.is_none());
}

#[test]
fn verified_without_user_id_is_rejected() {
    let parts = parts_with(&[("x-proxy-verified", "true")]);
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![]), "srv")
        .expect_err("missing user id rejected");
    assert!(err.to_string().contains("x-user-id"));
}

#[test]
fn verified_without_permissions_is_rejected() {
    let parts = parts_with(&[("x-proxy-verified", "true"), ("x-user-id", USER_ID)]);
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![]), "srv")
        .expect_err("missing permissions rejected");
    assert!(err.to_string().contains("x-user-permissions"));
}

#[test]
fn verified_with_unparseable_permissions_is_rejected() {
    let parts = parts_with(&[
        ("x-proxy-verified", "true"),
        ("x-user-id", USER_ID),
        ("x-user-permissions", "not-a-permission"),
    ]);
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![]), "srv")
        .expect_err("bad permissions rejected");
    assert!(err.to_string().contains("x-user-permissions"));
}

#[test]
fn insufficient_scope_is_rejected() {
    let parts = parts_with(&verified_headers());
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![Permission::Admin]), "srv")
        .expect_err("scope check enforced");
    assert!(err.to_string().contains("Insufficient permissions"));
}

#[test]
fn invalid_user_uuid_is_rejected() {
    let parts = parts_with(&[
        ("x-proxy-verified", "true"),
        ("x-user-id", "not-a-uuid"),
        ("x-user-permissions", "mcp"),
        ("authorization", "Bearer t"),
    ]);
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![Permission::Mcp]), "srv")
        .expect_err("uuid parse enforced");
    assert!(err.to_string().contains("Invalid user ID"));
}

#[test]
fn missing_bearer_is_rejected() {
    let parts = parts_with(&[
        ("x-proxy-verified", "true"),
        ("x-user-id", USER_ID),
        ("x-user-permissions", "mcp"),
    ]);
    let err = try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![Permission::Mcp]), "srv")
        .expect_err("bearer required");
    assert!(err.to_string().contains("Authorization Bearer"));
}

#[test]
fn verified_request_authenticates() {
    let parts = parts_with(&verified_headers());
    let result =
        try_proxy_verified_auth(Some(&parts), ctx(), &oauth(vec![Permission::User]), "srv")
            .expect("auth ok")
            .expect("short-circuits");

    let auth: AuthenticatedRequestContext = match result {
        AuthResult::Authenticated(auth) => auth,
        AuthResult::Anonymous(_) => panic!("expected authenticated result"),
    };
    assert_eq!(auth.token(), "proxied-token");
    assert_eq!(auth.context.user_id().to_string(), USER_ID);
}
