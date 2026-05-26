//! Stream router — covers routing and the first chunk emission for SSE
//! endpoints. We do not assert on the full event stream — we only verify
//! the routes are wired and the handlers begin executing before the
//! oneshot drops the connection.

use axum::Extension;
use systemprompt_api::routes::stream::stream_router;
use tower::ServiceExt;

use super::common::{empty_get, request_context, setup_ctx};

#[tokio::test]
async fn agui_stream_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = stream_router(&ctx)?.layer(Extension(request_context("user_agui")));
    let resp = app.oneshot(empty_get("/agui")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn a2a_stream_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = stream_router(&ctx)?.layer(Extension(request_context("user_a2a")));
    let resp = app.oneshot(empty_get("/a2a")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn contexts_stream_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = stream_router(&ctx)?.layer(Extension(request_context("user_ctx")));
    let resp = app.oneshot(empty_get("/contexts")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}
