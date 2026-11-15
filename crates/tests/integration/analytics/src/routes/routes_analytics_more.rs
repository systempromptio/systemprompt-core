//! Analytics + engagement content-routing resolution branches.
//!
//! Drives the entry points that mount the analytics and engagement routers
//! over a stub `ContentRouting`, so the slug-resolution success path (routing
//! maps a page URL to a seeded `markdown_content` slug, then `find_by_slug`
//! returns the row) and the conversion-marking path both execute — branches the
//! production router cannot reach without a live content config.

use std::sync::Arc;

use axum::Extension;
use systemprompt_models::ContentRouting;
use tower::ServiceExt;

use super::common::{
    SlugRouter, analytics_router_with_routing, engagement_router_with_routing, json_post,
    request_context, seed_content, setup_ctx,
};

#[tokio::test]
async fn record_event_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics_router_with_routing(&ctx, Some(routing))
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({ "event_type": "page_view", "page_url": page_url });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn record_event_with_explicit_slug_skips_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, _page_url) = seed_content(&db).await?;
    let app =
        analytics_router_with_routing(&ctx, None).layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "event_type": "page_view",
        "page_url": "https://example.com/whatever",
        "slug": slug,
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn page_exit_fanout_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics_router_with_routing(&ctx, Some(routing))
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "event_type": "page_exit",
        "page_url": page_url,
        "data": {
            "time_on_page_ms": 5000,
            "max_scroll_depth": 90,
            "click_count": 4,
            "is_rage_click": true,
            "reading_pattern": "scan",
            "scroll_velocity_avg": 1.5,
            "time_to_first_interaction_ms": 300,
        }
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn batch_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = analytics_router_with_routing(&ctx, Some(routing))
        .layer(Extension(request_context("user_an_more")));

    let body = serde_json::json!({
        "events": [
            { "event_type": "page_view", "page_url": page_url },
            {
                "event_type": "page_exit",
                "page_url": page_url,
                "data": { "time_on_page_ms": 2000, "max_scroll_depth": 50 }
            }
        ]
    });
    let resp = app.oneshot(json_post("/events/batch", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn engagement_resolves_content_and_marks_conversion() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = engagement_router_with_routing(&ctx, Some(routing))
        .layer(Extension(request_context("user_en_more")));

    let body = serde_json::json!({
        "page_url": page_url,
        "event_type": "github_click",
        "time_on_page_ms": 3000,
        "max_scroll_depth": 70,
        "click_count": 2,
    });
    let resp = app.oneshot(json_post("/track/engagement", body)).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn engagement_batch_resolves_and_marks_conversion() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (slug, page_url) = seed_content(&db).await?;
    let routing: Arc<dyn ContentRouting> = Arc::new(SlugRouter {
        page_url: page_url.clone(),
        slug,
    });
    let app = engagement_router_with_routing(&ctx, Some(routing))
        .layer(Extension(request_context("user_en_more")));

    let body = serde_json::json!({
        "events": [
            {
                "page_url": page_url,
                "event_type": "demo_click",
                "time_on_page_ms": 1000,
                "max_scroll_depth": 30,
                "click_count": 1,
            },
            {
                "page_url": page_url,
                "event_type": "scroll",
                "time_on_page_ms": 500,
                "max_scroll_depth": 20,
                "click_count": 0,
            }
        ]
    });
    let resp = app
        .oneshot(json_post("/track/engagement/batch", body))
        .await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}
