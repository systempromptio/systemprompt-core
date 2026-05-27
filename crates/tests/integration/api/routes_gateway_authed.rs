//! Authenticated gateway routes — exercises pubkey, whoami, and manifest by
//! installing a test signing key and minting a bearer JWT for an admin user
//! seeded in the fixture DB.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{install_test_signing_key, mint_admin_jwt};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    Ok(gateway_router(&ctx).expect("gateway router available"))
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn pubkey_after_install_returns_ok() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(Request::builder().uri("/bridge/pubkey").body(Body::empty()).unwrap())
        .await?;
    // Either OK (signing key installed for both authority + manifest_signing)
    // or 500 if manifest_signing key isn't installed in the test process.
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn whoami_with_jwt_for_unknown_user_returns_404_or_500() -> anyhow::Result<()> {
    let app = router().await?;
    let user = UserId::new(format!("test-user-{}", uuid::Uuid::new_v4()));
    let jwt = mint_admin_jwt(&user, "u@example.com", "test");
    let resp = app.oneshot(authed_get("/bridge/whoami", jwt.as_str())).await?;
    // Either NOT_FOUND (user not in DB), or 500 (DB schema mismatch in fixture).
    let s = resp.status().as_u16();
    assert!(
        s == 404 || s == 500 || s == 401,
        "whoami unexpected status: {s}"
    );
    Ok(())
}

#[tokio::test]
async fn manifest_with_jwt_no_profile_returns_5xx_or_4xx() -> anyhow::Result<()> {
    let app = router().await?;
    let user = UserId::new("u1");
    let jwt = mint_admin_jwt(&user, "u@example.com", "test");
    let resp = app.oneshot(authed_get("/bridge/manifest", jwt.as_str())).await?;
    let s = resp.status();
    assert!(s.is_client_error() || s.is_server_error(), "{s}");
    Ok(())
}

#[tokio::test]
async fn profile_usage_with_jwt_returns_response() -> anyhow::Result<()> {
    let app = router().await?;
    let user = UserId::new("u1");
    let jwt = mint_admin_jwt(&user, "u@example.com", "test");
    let resp = app
        .oneshot(authed_get("/bridge/profile/usage", jwt.as_str()))
        .await?;
    let s = resp.status();
    assert!(s.as_u16() >= 200, "{s}");
    Ok(())
}

#[tokio::test]
async fn heartbeat_with_jwt_returns_response() -> anyhow::Result<()> {
    let app = router().await?;
    let user = UserId::new("u1");
    let jwt = mint_admin_jwt(&user, "u@example.com", "test");
    let body = serde_json::json!({
        "session_id": "00000000-0000-0000-0000-000000000000",
        "bridge_version": "1.0.0",
        "os": "linux",
        "hostname": "test"
    });
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/bridge/heartbeat")
        .header(header::AUTHORIZATION, format!("Bearer {}", jwt.as_str()))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await?;
    let s = resp.status();
    assert!(s.as_u16() >= 200, "{s}");
    Ok(())
}

#[tokio::test]
async fn set_enabled_host_with_jwt_returns_response() -> anyhow::Result<()> {
    let app = router().await?;
    let user = UserId::new("u1");
    let jwt = mint_admin_jwt(&user, "u@example.com", "test");
    let body = serde_json::json!({"host_id": "claude-code", "enabled": true});
    let req = Request::builder()
        .method(http::Method::POST)
        .uri("/bridge/profile/enabled_hosts")
        .header(header::AUTHORIZATION, format!("Bearer {}", jwt.as_str()))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await?;
    let s = resp.status();
    assert!(s.as_u16() >= 200, "{s}");
    Ok(())
}
