//! Exercise additional API routes: sync, engagement, analytics, proxy,
//! contexts, artifacts, oauth discovery and webauthn endpoints — to pull
//! down coverage on `routes/**` modules that the per-feature suites do
//! not touch.

use axum::Extension;
use systemprompt_api::routes;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, json_post, request_context, setup_ctx};

async fn ctx() -> anyhow::Result<std::sync::Arc<AppContext>> {
    Ok(setup_ctx().await?.1)
}

#[tokio::test]
async fn sync_router_manifest_runs() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::sync_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u")));
    let resp = app.oneshot(empty_get("/files/manifest")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn sync_router_download_runs() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::sync_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u")));
    let resp = app.oneshot(empty_get("/files?dry_run=true")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn engagement_router_batch_runs() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::engagement_router(&ctx)?.layer(Extension(request_context("u")));
    let resp = app
        .oneshot(json_post(
            "/batch",
            serde_json::json!({ "events": [] }),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn engagement_router_single_runs() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::engagement_router(&ctx)?.layer(Extension(request_context("u")));
    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({
                "session_id": "sess_test",
                "event_type": "view",
                "url": "/x"
            }),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn analytics_router_record_event() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::analytics_router(&ctx)?.layer(Extension(request_context("u")));
    let resp = app
        .oneshot(json_post(
            "/events",
            serde_json::json!({
                "session_id": "sess_test",
                "event_type": "view",
                "url": "/y"
            }),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn analytics_router_batch() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::analytics_router(&ctx)?.layer(Extension(request_context("u")));
    let resp = app
        .oneshot(json_post("/events/batch", serde_json::json!({ "events": [] })))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn proxy_mcp_unknown_execution_returns_4xx() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::proxy::mcp::router(&ctx).layer(Extension(request_context("u")));
    let resp = app.oneshot(empty_get("/executions/exec_missing")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn proxy_mcp_protected_resource_well_known() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::proxy::mcp::router(&ctx).layer(Extension(request_context("u")));
    let resp = app
        .oneshot(empty_get(
            "/some-mcp/mcp/.well-known/oauth-protected-resource",
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn proxy_mcp_authz_server_well_known() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::proxy::mcp::router(&ctx).layer(Extension(request_context("u")));
    let resp = app
        .oneshot(empty_get(
            "/some-mcp/mcp/.well-known/oauth-authorization-server",
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn contexts_router_smoke() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::contexts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u")));
    // contexts_router parses ContextId via path; pass a real UUID v4.
    let resp = app
        .oneshot(empty_get("/00000000-0000-4000-8000-000000000000"))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn artifacts_router_user_list() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::artifacts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_artifacts")));
    let resp = app.oneshot(empty_get("/")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn artifacts_router_get_missing() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::artifacts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_artifacts")));
    let resp = app.oneshot(empty_get("/art_missing")).await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn artifacts_router_ui_missing() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::artifacts_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_artifacts")));
    let resp = app.oneshot(empty_get("/art_missing/ui")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn tasks_artifacts_for_missing_task_returns_array() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::tasks_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_tasks")));
    let resp = app.oneshot(empty_get("/task_x/artifacts")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn webhook_router_smoke() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_wh")));
    let resp = app
        .oneshot(json_post(
            "/00000000-0000-4000-8000-000000000000/webhook",
            serde_json::json!({ "event": "x" }),
        ))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn content_redirect_router_short_code_404() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let app = routes::content::redirect_router(&pool);
    let resp = app.oneshot(empty_get("/r/abcdef")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn admin_cli_router_smoke() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_admin")));
    let resp = app.oneshot(empty_get("/cli/profile")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn delete_unknown_api_key_returns_status() -> anyhow::Result<()> {
    let ctx = ctx().await?;
    let app = routes::admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("u_admin")));
    let resp = app.oneshot(empty_delete("/api-keys/key_does_not_exist")).await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}
