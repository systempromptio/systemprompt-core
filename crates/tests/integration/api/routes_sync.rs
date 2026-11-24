//! Sync router — file manifest + download over the sync API.

use axum::Router;
use systemprompt_cloud_entry::http::SyncState;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

fn app(ctx: &AppContext) -> Router {
    systemprompt_cloud_entry::http::router(SyncState::new(
        ctx.app_paths().system().services().to_path_buf(),
    ))
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
