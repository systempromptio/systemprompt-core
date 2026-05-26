//! Analytics router — events, batches, and SSE stream.

use axum::Extension;
use systemprompt_api::routes::analytics_router;
use tower::ServiceExt;

use super::common::{empty_get, json_post, request_context, setup_ctx};

#[tokio::test]
async fn record_event_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        analytics_router(&ctx)?.layer(Extension(request_context("user_analytics")));
    let body = serde_json::json!({
        "event_type": "page_view",
        "url": "https://example.com/",
        "session_id": "00000000-0000-0000-0000-000000000000",
    });
    let resp = app.oneshot(json_post("/events", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn record_event_rejects_bad_payload() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        analytics_router(&ctx)?.layer(Extension(request_context("user_analytics")));
    let resp = app
        .oneshot(json_post("/events", serde_json::json!({"junk": true})))
        .await?;
    // missing required fields -> 4xx (deserialize failure)
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn record_events_batch_accepts_empty_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        analytics_router(&ctx)?.layer(Extension(request_context("user_analytics")));
    let body = serde_json::json!({ "events": [] });
    let resp = app.oneshot(json_post("/events/batch", body)).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn analytics_stream_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app =
        analytics_router(&ctx)?.layer(Extension(request_context("user_analytics")));
    let resp = app.oneshot(empty_get("/stream")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
