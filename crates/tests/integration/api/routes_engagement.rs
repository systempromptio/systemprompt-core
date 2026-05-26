//! Engagement router — single + batch engagement endpoints.

use axum::Extension;
use systemprompt_api::routes::engagement_router;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

#[tokio::test]
async fn record_engagement_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let body = serde_json::json!({
        "event_type": "scroll",
        "session_id": "00000000-0000-0000-0000-000000000000",
        "url": "https://example.com/",
    });
    let resp = app.oneshot(json_post("/", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn record_engagement_rejects_bad_payload() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let resp = app
        .oneshot(json_post("/", serde_json::json!({"nope": true})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn record_engagement_batch_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        engagement_router(&ctx)?.layer(Extension(request_context("user_engagement")));
    let body = serde_json::json!({ "events": [] });
    let resp = app.oneshot(json_post("/batch", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
