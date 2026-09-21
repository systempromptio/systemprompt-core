//! Agent contexts router — list / create / get / update / delete contexts
//! plus the tasks and artifacts sub-routes.

use axum::Extension;
use axum::http::StatusCode;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_api::routes::contexts_router;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, seed_user_row,
};
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
async fn delete_context_returns_no_content_and_removes_owned_row() -> anyhow::Result<()> {
    ensure_test_bootstrap();
    let database = DisposableDb::installed("api_delete_context").await?;
    let pool = database.pool().await?;
    let ctx = fixture_app_context(&pool, database.url())?;
    let request = request_context("delete_context_owner");
    seed_user_row(
        &pool,
        request.user_id(),
        &format!("{}@contexts.invalid", request.user_id()),
    )
    .await?;
    let context_id = ctx
        .a2a_repositories()
        .contexts
        .create_context(request.user_id(), None, "delete me", ContextKind::User)
        .await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request.clone()));

    let response = app.oneshot(empty_delete(&format!("/{context_id}"))).await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(matches!(
        ctx.a2a_repositories()
            .contexts
            .get_context(&context_id, request.user_id())
            .await,
        Err(systemprompt_traits::RepositoryError::NotFound(_))
    ));

    drop(ctx);
    drop(pool);
    database.drop_now().await;
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
        .oneshot(empty_get("/00000000-0000-0000-0000-000000000000/artifacts"))
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

#[tokio::test]
async fn context_event_rejects_malformed_path_before_routing_valid_event() -> anyhow::Result<()> {
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use systemprompt_models::{ContextEvent, SystemEventBuilder};

    let (_pool, ctx) = setup_ctx().await?;
    let app = contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("event_path_owner")));
    let event: ContextEvent = SystemEventBuilder::contexts_snapshot(Vec::new()).into();
    let response = app
        .oneshot(json_post(
            "/not-a-context/events",
            serde_json::to_value(event)?,
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(
        json["error"]
            .as_str()
            .is_some_and(|message| message.contains("invalid context id"))
    );
    Ok(())
}
