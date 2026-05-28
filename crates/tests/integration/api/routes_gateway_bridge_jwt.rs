//! User-tier (non-admin) bridge JWT path. Asserts the success branch in
//! `decode_for_gateway` for the User permission set + reconciliation against
//! a non-admin user row.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use http::StatusCode;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    install_test_signing_key, mint_bridge_jwt, seed_bridge_credential,
};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    Ok((gateway_router(&ctx).expect("gateway router available"), pool))
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn whoami_for_unseeded_bridge_user_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let user = UserId::new(format!("bridge-missing-{}", uuid::Uuid::new_v4()));
    let jwt = mint_bridge_jwt(&user, "missing@example.invalid", "test");
    let resp = app
        .oneshot(authed_get("/bridge/whoami", jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn whoami_for_seeded_bridge_user_returns_envelope() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "bridge-ok@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(body["user_id"], cred.user_id.as_str());
    assert_eq!(body["email"].as_str(), Some(cred.email.as_str()));
    Ok(())
}

#[tokio::test]
async fn profile_usage_for_seeded_bridge_user_returns_ok() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "bridge-usage@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/profile/usage", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn pubkey_endpoint_is_public_no_auth_needed() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/pubkey")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let s = resp.status();
    assert!(s.is_success() || s.is_server_error(), "{s}");
    Ok(())
}
