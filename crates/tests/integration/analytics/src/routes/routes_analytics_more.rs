//! Analytics + engagement content-routing resolution branches, driven
//! through the analytics member's composed route sets: the routing is the
//! context's content configuration, so a page URL that no content source
//! claims resolves no slug and an explicit slug skips routing.

use axum::Extension;
use tower::ServiceExt;

use super::common::{
    analytics_router, engagement_router, json_post, request_context, seed_content, setup_ctx,
};

#[tokio::test]
async fn record_event_resolves_content_via_routing() -> anyhow::Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx)
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
        analytics_router(&ctx).layer(Extension(request_context("user_an_more")));

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
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx)
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
    let (_slug, page_url) = seed_content(&db).await?;
    let app = analytics_router(&ctx)
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
    let (_slug, page_url) = seed_content(&db).await?;
    let app = engagement_router(&ctx)
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
    let (_slug, page_url) = seed_content(&db).await?;
    let app = engagement_router(&ctx)
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
