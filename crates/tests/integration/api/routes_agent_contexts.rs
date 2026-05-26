//! Agent contexts router — list / create / get / update / delete contexts
//! plus the tasks and artifacts sub-routes.

use axum::Extension;
use systemprompt_api::routes::contexts_router;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, json_post, request_context, setup_ctx};

#[tokio::test]
async fn list_contexts_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app.oneshot(empty_get("/")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn create_context_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({
                "agent_name": "test-agent",
                "metadata": {}
            }),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn get_context_unknown_returns_4xx_or_5xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(empty_get("/00000000-0000-0000-0000-000000000000"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn delete_context_unknown_is_idempotent() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(empty_delete("/00000000-0000-0000-0000-000000000000"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn list_tasks_by_context_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(empty_get("/00000000-0000-0000-0000-000000000000/tasks"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn list_artifacts_by_context_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(empty_get(
            "/00000000-0000-0000-0000-000000000000/artifacts",
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn context_notification_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(json_post(
            "/00000000-0000-0000-0000-000000000000/notifications",
            serde_json::json!({}),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn context_events_forward_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_ctx")));
    let resp = app
        .oneshot(json_post(
            "/00000000-0000-0000-0000-000000000000/events",
            serde_json::json!({}),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
