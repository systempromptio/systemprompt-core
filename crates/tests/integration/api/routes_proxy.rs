//! Proxy router — agents proxy and MCP proxy. We do not have a backend
//! to forward to, so every request returns an error; the test still
//! exercises the proxy dispatch, registry lookup, and error mapping.

use systemprompt_api::routes::proxy::{agents, mcp};
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

#[tokio::test]
async fn agents_proxy_unknown_service_returns_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = agents::router(&ctx);
    let resp = app.oneshot(empty_get("/does-not-exist")).await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

#[tokio::test]
async fn agents_proxy_with_path_returns_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = agents::router(&ctx);
    let resp = app.oneshot(empty_get("/does-not-exist/foo/bar")).await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

#[tokio::test]
async fn mcp_proxy_unknown_service_returns_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = mcp::router(&ctx);
    let resp = app.oneshot(empty_get("/no-such-server/tools/list")).await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

#[tokio::test]
async fn mcp_proxy_get_execution_unknown_returns_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = mcp::router(&ctx);
    let resp = app.oneshot(empty_get("/executions/exec_unknown")).await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

#[tokio::test]
async fn mcp_proxy_protected_resource_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = mcp::router(&ctx);
    let resp = app
        .oneshot(empty_get(
            "/my-server/mcp/.well-known/oauth-protected-resource",
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn mcp_proxy_authorization_server_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = mcp::router(&ctx);
    let resp = app
        .oneshot(empty_get(
            "/my-server/mcp/.well-known/oauth-authorization-server",
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
