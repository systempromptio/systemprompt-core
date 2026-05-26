//! Content + content/links route handlers.

use axum::Extension;
use systemprompt_api::routes::content;
use tower::ServiceExt;

use super::common::{empty_get, json_post, request_context, setup_ctx};

#[tokio::test]
async fn list_links_returns_payload() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app.oneshot(empty_get("/links")).await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_link_performance_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app
        .oneshot(empty_get("/links/some_link_id/performance"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn get_link_clicks_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app
        .oneshot(empty_get("/links/some_link_id/clicks"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn get_campaign_performance_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app
        .oneshot(empty_get("/links/campaigns/camp_xyz/performance"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn get_journey_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app.oneshot(empty_get("/links/journey")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn list_content_by_source_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let resp = app.oneshot(empty_get("/some_source")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn query_handler_accepts_post() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::public_router(&ctx).layer(Extension(request_context("user_links")));
    let body = serde_json::json!({ "query": "test", "limit": 10 });
    let resp = app.oneshot(json_post("/query", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn generate_link_handler_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = content::authenticated_router(&ctx).layer(Extension(request_context("user_links")));
    let body = serde_json::json!({
        "url": "https://example.com",
        "campaign_id": "test",
        "source": "test"
    });
    let resp = app.oneshot(json_post("/links/generate", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
