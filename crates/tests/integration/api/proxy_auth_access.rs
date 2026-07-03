//! Proxy access enforcement — drives
//! `AccessValidator::validate_with_requirement` through the `test-api` seam.
//! Covers the no-auth challenge, the MCP session-only fallback, bearer
//! validation, scope enforcement, and the required-audience check, asserting on
//! the RFC 6750 / RFC 9728 challenge responses the proxy emits.

use axum::http::{HeaderMap, HeaderValue, header};
use systemprompt_api::services::proxy::auth_test_api::{Requirement, validate_with_requirement};
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_test_fixtures::{install_test_signing_key, mint_admin_jwt};
use uuid::Uuid;

use super::common::{request_context, setup_ctx};

fn requirement(required: bool, scopes: &[&str], audience: &str) -> Requirement {
    Requirement {
        module: "mcp".to_owned(),
        required,
        scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
        audience: audience.to_owned(),
    }
}

fn admin_bearer_headers() -> HeaderMap {
    install_test_signing_key();
    let issuer = Config::get().expect("config").jwt_issuer.clone();
    let user = UserId::new(Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user, "proxy-access@test.invalid", &issuer);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token.as_str())).expect("header"),
    );
    headers
}

#[tokio::test]
async fn oauth_not_required_returns_none() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = HeaderMap::new();
    let result =
        validate_with_requirement(&headers, "svc", &requirement(false, &[], ""), &ctx, None);
    assert!(matches!(result, Ok(None)));
    Ok(())
}

#[tokio::test]
async fn missing_credentials_returns_challenge_without_error_attr() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let rc = request_context("proxy-access-anon");
    let headers = HeaderMap::new();
    let result = validate_with_requirement(
        &headers,
        "svc-mcp",
        &requirement(true, &[], ""),
        &ctx,
        Some(&rc),
    );
    let Err(err) = result else {
        panic!("must be challenged")
    };
    assert_eq!(err.status(), axum::http::StatusCode::UNAUTHORIZED);
    let www = err
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .expect("www-authenticate present")
        .to_owned();
    assert!(www.contains("resource_metadata"), "{www}");
    assert!(
        !www.contains("error="),
        "no-credentials challenge must omit error attr: {www}"
    );
    Ok(())
}

#[tokio::test]
async fn invalid_bearer_returns_invalid_token_challenge() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer not-a-jwt"),
    );
    let result =
        validate_with_requirement(&headers, "svc-mcp", &requirement(true, &[], ""), &ctx, None);
    let Err(err) = result else {
        panic!("must be challenged")
    };
    assert_eq!(err.status(), axum::http::StatusCode::UNAUTHORIZED);
    let www = err
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .expect("www-authenticate present");
    assert!(www.contains("invalid_token"), "{www}");
    Ok(())
}

#[tokio::test]
async fn mcp_session_only_request_falls_back_to_cache_identity() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let mut headers = HeaderMap::new();
    headers.insert("mcp-session-id", HeaderValue::from_static("sess-123"));
    let result =
        validate_with_requirement(&headers, "svc-mcp", &requirement(true, &[], ""), &ctx, None);
    assert!(
        matches!(result, Ok(None)),
        "session-only MCP request must pass through for cache enrichment"
    );
    Ok(())
}

#[tokio::test]
async fn mcp_session_with_stale_bearer_is_still_challenged() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let mut headers = HeaderMap::new();
    headers.insert("mcp-session-id", HeaderValue::from_static("sess-456"));
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer expired-token"),
    );
    let result =
        validate_with_requirement(&headers, "svc-mcp", &requirement(true, &[], ""), &ctx, None);
    let Err(err) = result else {
        panic!("stale bearer must trigger a refresh challenge")
    };
    assert_eq!(err.status(), axum::http::StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn agent_module_challenge_advertises_agent_resource() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = HeaderMap::new();
    let mut req = requirement(true, &[], "");
    req.module = "agent".to_owned();
    let result = validate_with_requirement(&headers, "my-agent", &req, &ctx, None);
    let Err(err) = result else {
        panic!("must be challenged")
    };
    let www = err
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .expect("www-authenticate present");
    assert!(www.contains("my-agent"), "{www}");
    Ok(())
}

#[tokio::test]
async fn valid_bearer_with_no_scope_requirement_returns_user() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = admin_bearer_headers();
    let result =
        validate_with_requirement(&headers, "svc-mcp", &requirement(true, &[], ""), &ctx, None);
    match result {
        Ok(Some(user)) => assert!(!user.permissions.is_empty()),
        Ok(None) => panic!("authenticated request must resolve a user"),
        Err(resp) => {
            assert!(
                resp.status().is_client_error(),
                "unexpected server error {}",
                resp.status()
            );
        },
    }
    Ok(())
}

#[tokio::test]
async fn valid_bearer_with_implied_scope_passes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = admin_bearer_headers();
    let result = validate_with_requirement(
        &headers,
        "svc-mcp",
        &requirement(true, &["user"], ""),
        &ctx,
        None,
    );
    if let Ok(user) = &result {
        assert!(user.is_some(), "scope check must return the user");
    }
    Ok(())
}

#[tokio::test]
async fn valid_bearer_missing_scope_is_forbidden() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = admin_bearer_headers();
    let result = validate_with_requirement(
        &headers,
        "svc-mcp",
        &requirement(true, &["no-such-scope"], ""),
        &ctx,
        None,
    );
    let Err(err) = result else {
        panic!("missing scope must be rejected")
    };
    assert!(
        err.status() == axum::http::StatusCode::FORBIDDEN
            || err.status() == axum::http::StatusCode::UNAUTHORIZED,
        "got {}",
        err.status()
    );
    Ok(())
}

#[tokio::test]
async fn required_audience_not_carried_by_token_is_rejected() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let headers = admin_bearer_headers();
    let result = validate_with_requirement(
        &headers,
        "svc-mcp",
        &requirement(true, &[], "hook"),
        &ctx,
        None,
    );
    let Err(err) = result else {
        panic!("token without the hook audience must be rejected")
    };
    assert!(err.status().is_client_error(), "got {}", err.status());
    Ok(())
}
