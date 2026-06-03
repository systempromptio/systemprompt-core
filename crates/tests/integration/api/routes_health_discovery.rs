//! Server discovery + health surfaces — `handle_root_discovery`,
//! `handle_core_discovery`, `handle_agents_discovery`, `handle_mcp_discovery`,
//! and `handle_health`. These are the unauthenticated discovery routes mounted
//! by `discovery_router`; we mount the handlers directly (the full
//! `discovery_router` additionally wants a process-global Prometheus handle)
//! and assert each returns a JSON body with the expected shape.

use axum::Router;
use axum::routing::get;
use systemprompt_api::services::server::builder::{
    handle_agents_discovery, handle_core_discovery, handle_health, handle_mcp_discovery,
    handle_root_discovery,
};
use systemprompt_models::modules::ApiPaths;
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, setup_ctx};

async fn discovery_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    Ok(Router::new()
        .route(ApiPaths::DISCOVERY, get(handle_root_discovery))
        .route(ApiPaths::HEALTH, get(handle_health))
        .route("/health", get(handle_health))
        .route(ApiPaths::CORE_BASE, get(handle_core_discovery))
        .route(ApiPaths::AGENTS_BASE, get(handle_agents_discovery))
        .route(ApiPaths::MCP_BASE, get(handle_mcp_discovery))
        .with_state((*ctx).clone()))
}

#[tokio::test]
async fn health_returns_status_field() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get("/health")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.as_u16() == 200 || status.as_u16() == 503, "{status}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        v.get("status").and_then(|s| s.as_str()).is_some(),
        "health body must carry a status field: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn root_discovery_returns_endpoints() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::DISCOVERY)).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success(), "{status} {body}");
    let v: serde_json::Value = serde_json::from_str(&body)?;
    let data = v.get("data").unwrap_or(&v);
    assert!(
        data.get("endpoints").is_some(),
        "root discovery missing endpoints: {body}"
    );
    assert!(
        data.get("version").is_some(),
        "root discovery missing version: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn core_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::CORE_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}

#[tokio::test]
async fn agents_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::AGENTS_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}

#[tokio::test]
async fn mcp_discovery_runs() -> anyhow::Result<()> {
    let app = discovery_app().await?;
    let resp = app.oneshot(empty_get(ApiPaths::MCP_BASE)).await?;
    let status = resp.status();
    assert!(status.is_success(), "{status}");
    Ok(())
}
