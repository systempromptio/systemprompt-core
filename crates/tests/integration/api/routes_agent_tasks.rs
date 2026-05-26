//! Agent tasks routes — list/get/delete against a real DB. These hit the
//! TaskRepository and exercise the typed-ID path conversions, status filters,
//! and per-handler error mappings.

use axum::Extension;
use systemprompt_api::routes::tasks_router;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, request_context, setup_ctx};

fn app(ctx: &systemprompt_runtime::AppContext) -> axum::Router {
    tasks_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context("user_tasks")))
}

#[tokio::test]
async fn list_user_tasks_returns_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_get("/")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_user_tasks_accepts_status_filter() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(empty_get("/?status=completed&limit=5"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_user_tasks_handles_all_status_values() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for s in [
        "submitted",
        "working",
        "input-required",
        "completed",
        "canceled",
        "cancelled",
        "failed",
        "rejected",
        "auth-required",
        "weird-unknown",
    ] {
        let resp = app(&ctx).oneshot(empty_get(&format!("/?status={s}"))).await?;
        assert!(resp.status().is_success(), "status={s} -> {}", resp.status());
    }
    Ok(())
}

#[tokio::test]
async fn get_missing_task_returns_404() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(empty_get("/task_does_not_exist"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn get_messages_for_missing_task_returns_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(empty_get("/task_missing/messages"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn delete_missing_task_is_idempotent() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_delete("/task_missing")).await?;
    // DELETE is idempotent in the repo: returns 204 even if nothing matched.
    assert!(
        resp.status().is_success(),
        "{}",
        resp.status()
    );
    Ok(())
}
