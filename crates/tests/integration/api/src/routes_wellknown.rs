//! `/.well-known/*` endpoints.

use systemprompt_api::routes::wellknown_router;
use tower::ServiceExt;

use super::common::{empty_get, read_json, setup_ctx};

#[tokio::test]
async fn the_apple_association_document_is_not_found_when_no_ios_app_is_vouched_for()
-> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx)?;
    let resp = app
        .oneshot(empty_get("/.well-known/apple-app-site-association"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    let body = read_json(resp).await?;
    assert_eq!(
        body["error_key"],
        "apple_app_site_association_not_configured"
    );
    Ok(())
}

#[tokio::test]
async fn the_asset_links_document_is_not_found_when_no_android_app_is_vouched_for()
-> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx)?;
    let resp = app
        .oneshot(empty_get("/.well-known/assetlinks.json"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    let body = read_json(resp).await?;
    assert_eq!(body["error_key"], "asset_links_not_configured");
    Ok(())
}

#[tokio::test]
async fn jwks_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx)?;

    let resp = app.oneshot(empty_get("/.well-known/jwks.json")).await?;
    let status = resp.status();
    assert!(status.as_u16() == 200 || status.as_u16() >= 500, "{status}");
    Ok(())
}

#[tokio::test]
async fn default_agent_card_returns_error_without_profile() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = crate::common::agent_cards_app(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/agent-card.json"))
        .await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

#[tokio::test]
async fn bridge_discovery_is_not_found_without_a_bridge_instance_block() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx)?;
    let resp = app
        .oneshot(empty_get("/.well-known/systemprompt-bridge"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}
