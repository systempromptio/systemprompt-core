//! Exercise the marketplace HTTP routes through `oneshot`.
//!
//! These tests do not set up a profile/services config, so handlers that call
//! `ConfigLoader::load()` will return 500. That is intentional — the routing,
//! state plumbing, header negotiation, and error-mapping paths all execute.

use axum::Router;
use systemprompt_api::routes::marketplace;
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

async fn router() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    Ok(marketplace::router().with_state((*ctx).clone()))
}

#[tokio::test]
async fn marketplace_json_returns_error_without_profile() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/marketplace.json")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn get_marketplace_by_id_returns_error_without_profile() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/marketplaces/default")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn get_marketplace_yaml_rejects_traversal() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(empty_get("/marketplaces/..%2Fetc/manifest.yaml"))
        .await?;
    assert!(resp.status().is_client_error());
    Ok(())
}

#[tokio::test]
async fn get_marketplace_yaml_missing_returns_404() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(empty_get("/marketplaces/no-such-id/manifest.yaml"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn serve_plugin_file_missing_plugin_returns_404() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(empty_get("/plugins/no-such-plugin/readme.md"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}
