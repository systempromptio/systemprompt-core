//! Unauthenticated requests to `/api/v1/mcp/*` must NOT be rejected with the
//! generic `caller type 'anon' is not authorized for this route` 403 emitted
//! by `authz_gate`.
//!
//! The proxy handler answers them with a spec-compliant 401 +
//! `WWW-Authenticate: Bearer resource_metadata="…"` (RFC 9728). A role gate
//! mounted above it would short-circuit the request into a generic 403, and
//! spec-compliant MCP clients (Cowork, Claude Code, etc.) only start OAuth
//! discovery on a 401 carrying the challenge.
//!
//! This test drives the FULL mounted router (the same one `setup_api_server`
//! ships in production), POSTs to an MCP path with no `Authorization` header,
//! and asserts the response is NOT the generic 403 body. Pair with the unit
//! test `unit/entry/api/src/middleware/authz_policy.rs` (gate-level coverage).

use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};
use tower::ServiceExt;

async fn boot_full_router() -> anyhow::Result<axum::Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;

    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let ctx = fixture_app_context_with_config(&pool, config)?;
    let router = setup_api_server(
        &ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok(router)
}

async fn body_text(resp: axum::http::Response<Body>) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body collect");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn unauthenticated_mcp_post_does_not_hit_generic_anon_403() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;

    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "MCP route mount regressed to a gate that excludes Anon. The proxy handler \
         (services/proxy/auth.rs) cannot emit its RFC 9728 401 challenge if the gate 403s first. \
         status={status} body={body}"
    );
    Ok(())
}

#[tokio::test]
async fn unauthenticated_mcp_post_emits_clean_no_credentials_challenge() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .map(|v| v.to_str().unwrap_or_default().to_owned());
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected 401 challenge, got {status} body={body}"
    );
    let www_auth = www_auth.expect("WWW-Authenticate header present on 401");
    assert!(
        www_auth.starts_with("Bearer "),
        "WWW-Authenticate must be a Bearer challenge, got: {www_auth}"
    );
    assert!(
        www_auth.contains("resource_metadata="),
        "WWW-Authenticate must carry resource_metadata link (RFC 9728), got: {www_auth}"
    );
    assert!(
        !www_auth.contains("error="),
        "RFC 6750 §3 violation: no-credentials challenge must omit error= so clients begin OAuth \
         discovery rather than treat the request as rejected. WWW-Authenticate={www_auth}"
    );
    assert!(
        !body.contains("\"error\""),
        "no-credentials 401 body must not advertise an OAuth error code; got: {body}"
    );
    assert!(
        !body.contains("authorization_url"),
        "non-standard `authorization_url` field must not appear in the 401 body — authorization \
         servers are advertised via the `authorization_servers` array in oauth-protected-resource \
         metadata. body={body}"
    );
    Ok(())
}

#[tokio::test]
async fn mcp_post_with_bad_bearer_emits_invalid_token_challenge() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer not-a-real-token")
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .map(|v| v.to_str().unwrap_or_default().to_owned());
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected 401, got {status} body={body}"
    );
    let www_auth = www_auth.expect("WWW-Authenticate header present on 401");
    assert!(
        www_auth.contains("error=\"invalid_token\""),
        "bad-token challenge must advertise error=invalid_token per RFC 6750 §3.1, got: {www_auth}"
    );
    assert!(
        www_auth.contains("resource_metadata="),
        "WWW-Authenticate must still carry resource_metadata for client recovery, got: {www_auth}"
    );
    assert!(
        body.contains("\"error\":\"invalid_token\""),
        "bad-token 401 body must include OAuth error code, got: {body}"
    );
    assert!(
        !body.contains("authorization_url"),
        "non-standard `authorization_url` field must not appear in the 401 body; body={body}"
    );
    Ok(())
}

#[tokio::test]
async fn unauthenticated_mcp_get_does_not_hit_generic_anon_403() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/mcp/__regression__/mcp")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;
    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "MCP GET regressed to anon-denial gate. status={status} body={body}"
    );
    Ok(())
}

#[tokio::test]
async fn unauthenticated_mcp_registry_get_is_not_anon_denied() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/mcp-registry")
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;
    assert!(
        !body.contains("'anon' is not authorized for this route"),
        "mcp-registry regressed to anon-denial gate. status={status} body={body}"
    );
    Ok(())
}

#[tokio::test]
async fn unauthenticated_execution_lookup_is_refused() -> anyhow::Result<()> {
    let app = boot_full_router().await?;
    let mut req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/mcp/executions/{}", uuid::Uuid::new_v4()))
        .header(header::HOST, "127.0.0.1")
        .body(Body::empty())?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    let resp = app.oneshot(req).await?;
    let (status, body) = body_text(resp).await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "execution lookup must require authentication. status={status} body={body}"
    );
    Ok(())
}
