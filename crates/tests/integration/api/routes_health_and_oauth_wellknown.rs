//! `handle_health` and oauth `wellknown_routes` — these handlers query the
//! DB and depend on `Config`, so we mount them against the fixture context.

use axum::Router;
use axum::routing::get;
use systemprompt_api::routes::oauth::wellknown_routes;
use systemprompt_test_fixtures::ensure_config;
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

#[tokio::test]
async fn handle_health_returns_status() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app: Router = Router::new()
        .route(
            "/health",
            get(systemprompt_api::services::server::builder::handle_health),
        )
        .layer(axum::Extension(std::sync::Arc::new(
            systemprompt_api::services::server::SchedulerHealth::new(),
        )))
        .with_state((*ctx).clone());
    let resp = app.oneshot(empty_get("/health")).await?;
    let status = resp.status().as_u16();
    assert!(status == 200 || status == 503, "{status}");
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_oauth_authz_server() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-authorization-server"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_openid_config() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/openid-configuration"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_protected_resource() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/oauth-protected-resource"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn oauth_wellknown_routes_protected_resource_with_path() -> anyhow::Result<()> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_routes(&ctx);
    let resp = app
        .oneshot(empty_get(
            "/.well-known/oauth-protected-resource/mcp/foo/mcp",
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}
