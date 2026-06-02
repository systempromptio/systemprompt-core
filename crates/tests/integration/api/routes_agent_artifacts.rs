//! Agent artifact retrieval routes through `artifacts_router`.
//!
//! Drives `list_artifacts_by_user`, `get_artifact`, and `get_artifact_ui`
//! against a real DB with an injected `RequestContext` (the full router injects
//! this via the jwt-context middleware; the bare router test layers it
//! directly, matching `routes_agent_tasks`). The list path runs the user-scoped
//! repository query; the single-artifact and UI paths exercise the
//! ownership-validation and not-found branches.

use axum::Extension;
use systemprompt_api::routes::artifacts_router;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, request_context, setup_ctx};

fn app_for(ctx: &AppContext, user: &str) -> axum::Router {
    artifacts_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(user)))
}

#[tokio::test]
async fn list_artifacts_by_user_returns_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str()).oneshot(empty_get("/")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_artifacts_by_user_accepts_limit() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/?limit=10"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_unknown_artifact_returns_4xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/artifact_does_not_exist"))
        .await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_unknown_artifact_ui_returns_4xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/artifact_does_not_exist/ui"))
        .await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}
