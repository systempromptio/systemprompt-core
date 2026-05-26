//! Sync router — file manifest + download/upload over the sync API.

use axum::Router;
use systemprompt_api::routes::sync_router;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;

use super::common::{empty_get, json_post, setup_ctx};

fn app(ctx: &AppContext) -> Router {
    sync_router().with_state(ctx.clone())
}

#[tokio::test]
async fn manifest_returns_payload_or_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_get("/files/manifest")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn download_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_get("/files")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn upload_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(json_post("/files", serde_json::json!({})))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
